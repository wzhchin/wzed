use gpui::*;
use gpui::prelude::FluentBuilder as _;

pub(crate) struct TabInfo {
    pub index: usize,
    pub title: SharedString,
    pub is_active: bool,
    pub is_dirty: bool,
    pub group: Option<SharedString>,
}

#[derive(Clone)]
pub(crate) struct DraggedTab {
    pub index: usize,
    pub title: SharedString,
}

impl Render for DraggedTab {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px(px(10.0))
            .py(px(6.0))
            .bg(gpui::hsla(0.0, 0.0, 0.2, 0.9))
            .border_1()
            .border_color(gpui::hsla(220.0, 0.8, 0.6, 1.0))
            .rounded(px(4.0))
            .text_size(px(13.0))
            .text_color(gpui::hsla(0.0, 0.0, 0.9, 1.0))
            .child(self.title.clone())
    }
}

pub(crate) fn render_tab_list(
    tabs: &[TabInfo],
    scroll_handle: &ScrollHandle,
    cx: &mut Context<crate::workspace::LiteWorkspace>,
) -> impl IntoElement {
    let mut children: Vec<AnyElement> = Vec::new();
    let mut last_group: Option<&SharedString> = None;
    let mut active_child_index = 0usize;
    let mut child_index = 0usize;

    for tab in tabs {
        if let Some(ref group) = tab.group {
            if last_group != Some(group) {
                children.push(
                    div()
                        .px(px(10.0))
                        .py(px(3.0))
                        .text_size(px(10.0))
                        .text_color(gpui::hsla(0.0, 0.0, 0.4, 1.0))
                        .border_t_1()
                        .border_color(gpui::hsla(0.0, 0.0, 0.12, 1.0))
                        .child(group.clone())
                        .into_any_element(),
                );
                child_index += 1;
                last_group = Some(group);
            }
        } else {
            last_group = None;
        }

        if tab.is_active {
            active_child_index = child_index;
        }

        let idx = tab.index;
        let active = tab.is_active;
        let dirty = tab.is_dirty;
        let title = tab.title.clone();

        let dragged = DraggedTab {
            index: idx,
            title: title.clone(),
        };

        let mut tab_el = div()
            .id(ElementId::Name(format!("tab-{idx}").into()))
            .flex()
            .items_center()
            .px(px(10.0))
            .py(px(6.0))
            .w_full()
            .cursor_pointer()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .flex_1()
                    .child(
                        div()
                            .text_size(px(13.0))
                            .text_color(if active {
                                gpui::hsla(0.0, 0.0, 0.9, 1.0)
                            } else {
                                gpui::hsla(0.0, 0.0, 0.6, 1.0)
                            })
                            .text_ellipsis()
                            .child(title),
                    )
                    .when(dirty, |el| {
                        el.child(
                            div()
                                .ml(px(4.0))
                                .size(px(6.0))
                                .rounded_full()
                                .bg(gpui::hsla(220.0, 0.8, 0.6, 1.0)),
                        )
                    }),
            )
            .on_click(cx.listener(move |workspace, _, _window, cx| {
                workspace.active = idx;
                cx.notify();
            }))
            .on_drag(dragged, |drag: &DraggedTab, _position, _window, cx| {
                cx.new(|_| drag.clone())
            })
            .on_drop(cx.listener(move |this, dragged: &DraggedTab, _window, cx| {
                let from = dragged.index;
                let to = idx;
                if from == to || from >= this.tabs.len() || to >= this.tabs.len() {
                    return;
                }
                let active_id = this.tabs[this.active].editor.entity_id();
                let tab = this.tabs.remove(from);
                this.tabs.insert(to, tab);
                this.active = this
                    .tabs
                    .iter()
                    .position(|t| t.editor.entity_id() == active_id)
                    .unwrap_or(0);
                cx.notify();
            }));

        if active {
            tab_el = tab_el
                .bg(gpui::hsla(0.0, 0.0, 0.18, 1.0))
                .border_l_2()
                .border_color(gpui::hsla(220.0, 0.8, 0.6, 1.0));
        } else {
            tab_el = tab_el.hover(|s| s.bg(gpui::hsla(0.0, 0.0, 0.13, 1.0)));
        }

        children.push(tab_el.into_any_element());
        child_index += 1;
    }

    scroll_handle.scroll_to_item(active_child_index);
    div()
        .id("tab-list")
        .track_scroll(scroll_handle)
        .flex()
        .flex_col()
        .flex_1()
        .overflow_y_scroll()
        .children(children)
}
