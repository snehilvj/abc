//! Drawing the file explorer: tui-tree-widget draws the rows, two columns per
//! level; a folder's own icon shows whether it's open, and files carry their
//! type's icon. Icons follow `label`'s rules.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::StatefulWidget;
use tui_tree_widget::{Tree, TreeItem};

use super::label::{badge, blank, labelled};
use crate::explorer::{Explorer, Listing, Node, NodePath};
use crate::icons;
use crate::theme::Theme;

pub fn draw(buf: &mut Buffer, area: Rect, listing: Option<&Listing>, focused: bool, theme: &Theme) {
    let message = |buf: &mut Buffer, lines: Vec<Vec<Span>>| {
        // Messages line up with the tree's names: one column in, as the tree's rows are.
        for (i, spans) in lines.into_iter().enumerate() {
            buf.set_line(
                area.x + 1,
                area.y + i as u16,
                &Line::from(spans),
                area.width.saturating_sub(1),
            );
        }
    };
    match listing {
        None | Some(Listing::Loading) => message(
            buf,
            vec![labelled(
                icons::sync(),
                theme.dim(),
                "Loading files…",
                theme.dim(),
                theme,
            )],
        ),
        Some(Listing::Failed(why)) => message(
            buf,
            vec![
                badge(icons::failure(), "Can't list files", theme.error, theme),
                vec![],
                vec![Span::styled(why.as_str(), theme.text())],
            ],
        ),
        Some(Listing::Ready(explorer)) if explorer.is_empty() => message(
            buf,
            vec![
                vec![Span::styled("No files.", theme.text())],
                vec![],
                vec![Span::styled("The worktree is empty, or", theme.dim())],
                vec![Span::styled("everything in it is ignored.", theme.dim())],
            ],
        ),
        Some(Listing::Ready(explorer)) => tree(buf, area, explorer, focused, theme),
    }
}

fn tree(buf: &mut Buffer, area: Rect, explorer: &Explorer, focused: bool, theme: &Theme) {
    let mut cache = explorer.items.borrow_mut();
    let fresh = matches!(&*cache, Some((t, open, _)) if t == theme && *open == explorer.open);
    if !fresh {
        let items = items(
            &explorer.tree,
            &mut Vec::new(),
            explorer.open.as_ref(),
            theme,
        );
        *cache = Some((theme.clone(), explorer.open.clone(), items));
    }
    let Some((_, _, items)) = &*cache else { return };
    // A folder's open/closed icon is drawn as tui-tree-widget's node symbol, in the folder's colour.
    let closed = format!("{}{}", icons::folder(), blank(theme.icon_gap).content);
    let open = format!("{}{}", icons::folder_open(), blank(theme.icon_gap).content);
    let highlight = if focused {
        Style::new().bg(theme.selection)
    } else {
        Style::new()
    };
    let Ok(widget) = Tree::new(items) else { return };
    let widget = widget
        .node_closed_symbol(&closed)
        .node_open_symbol(&open)
        .node_no_children_symbol("")
        // A one-column margin that the selection's background still covers.
        .highlight_symbol(" ")
        .highlight_style(highlight);
    StatefulWidget::render(widget, area, buf, &mut explorer.state.borrow_mut());
}

fn items(
    nodes: &[Node],
    prefix: &mut NodePath,
    open: Option<&NodePath>,
    theme: &Theme,
) -> Vec<TreeItem<'static, String>> {
    let folder = theme.style(theme.folder);
    nodes
        .iter()
        .map(|node| {
            prefix.push(node.name.clone());
            let item = match &node.children {
                Some(children) => {
                    let text = Text::from(Line::styled(
                        node.name.clone(),
                        folder.add_modifier(Modifier::BOLD),
                    ))
                    .style(folder);
                    let children = items(children, prefix, open, theme);
                    TreeItem::new(node.name.clone(), text, children)
                        .expect("names are unique within a folder")
                }
                None => {
                    let (icon, color) = icons::for_file(&node.name, theme);
                    let name = if open == Some(prefix) {
                        theme.accent().add_modifier(Modifier::BOLD)
                    } else {
                        theme.text()
                    };
                    let line = labelled(
                        icon.to_string(),
                        theme.style(color),
                        node.name.clone(),
                        name,
                        theme,
                    );
                    TreeItem::new_leaf(node.name.clone(), Line::from(line))
                }
            };
            prefix.pop();
            item
        })
        .collect()
}
