//! A simple pop-over interface, based somewhat on tooltip and pick_list to
//! help with stuff like positioning

use iced::advanced::widget::{Operation, Tree, Widget};
use iced::advanced::{Clipboard, Shell, layout, overlay, renderer};
use iced::event::Event;
use iced::{Element, Length, Point, Rectangle, Size, Vector, mouse};

/// A widget that anchors a popup `content` element to a `trigger` element.
pub struct Popover<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer>
where
    Renderer: renderer::Renderer,
{
    // children[0] = trigger, children[1] = popup content
    children: Vec<Element<'a, Message, Theme, Renderer>>,
    is_open: bool,
    on_close: Option<Message>,
    gap: f32,
}

impl<'a, Message, Theme, Renderer> Popover<'a, Message, Theme, Renderer>
where
    Renderer: renderer::Renderer,
    Message: Clone,
{
    pub fn new(
        trigger: impl Into<Element<'a, Message, Theme, Renderer>>,
        content: impl Into<Element<'a, Message, Theme, Renderer>>,
        is_open: bool,
    ) -> Self {
        Self {
            children: vec![trigger.into(), content.into()],
            is_open,
            on_close: None,
            gap: 2.0,
        }
    }

    /// Message sent when the user clicks outside the popup while it's open.
    pub fn on_close(mut self, message: Message) -> Self {
        self.on_close = Some(message);
        self
    }

    /// Vertical gap between the trigger and the popup, in pixels.
    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = gap;
        self
    }
}

impl<'a, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Popover<'a, Message, Theme, Renderer>
where
    Renderer: renderer::Renderer,
    Message: Clone,
{
    fn size(&self) -> Size<Length> {
        self.children[0].as_widget().size()
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.children[0]
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: layout::Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.children[0].as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    fn children(&self) -> Vec<Tree> {
        self.children.iter().map(Tree::new).collect()
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(&self.children);
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: layout::Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        self.children[0].as_widget_mut().operate(
            &mut tree.children[0],
            layout,
            renderer,
            operation,
        );
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: layout::Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        self.children[0].as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: layout::Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.children[0].as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: layout::Layout<'b>,
        _renderer: &Renderer,
        _viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        if !self.is_open {
            return None;
        }

        let trigger_bounds = layout.bounds() + translation;
        let content = &mut self.children[1];
        let content_tree = &mut tree.children[1];

        Some(overlay::Element::new(Box::new(PopupOverlay {
            content,
            tree: content_tree,
            trigger_bounds,
            on_close: self.on_close.clone(),
            gap: self.gap,
        })))
    }
}

struct PopupOverlay<'a, 'b, Message, Theme, Renderer> {
    content: &'a mut Element<'b, Message, Theme, Renderer>,
    tree: &'a mut Tree,
    trigger_bounds: Rectangle,
    on_close: Option<Message>,
    gap: f32,
}

impl<Message, Theme, Renderer> overlay::Overlay<Message, Theme, Renderer>
    for PopupOverlay<'_, '_, Message, Theme, Renderer>
where
    Renderer: renderer::Renderer,
    Message: Clone,
{
    fn layout(&mut self, renderer: &Renderer, bounds: Size) -> layout::Node {
        let limits = layout::Limits::new(Size::ZERO, bounds);

        let mut node = self
            .content
            .as_widget_mut()
            .layout(self.tree, renderer, &limits);

        let size = node.size();

        // Anchor just below the trigger by default.
        let mut x = self.trigger_bounds.x;
        let mut y = self.trigger_bounds.y + self.trigger_bounds.height + self.gap;

        // Keep it on screen: nudge left, and flip above the trigger if
        // there isn't enough room below.
        if x + size.width > bounds.width {
            x = (bounds.width - size.width).max(0.0);
        }
        if y + size.height > bounds.height {
            y = (self.trigger_bounds.y - size.height - self.gap).max(0.0);
        }

        node = node.move_to(Point::new(x, y));
        node
    }

    fn draw(
        &self,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: layout::Layout<'_>,
        cursor: mouse::Cursor,
    ) {
        self.content.as_widget().draw(
            self.tree,
            renderer,
            theme,
            style,
            layout,
            cursor,
            &layout.bounds(),
        );
    }

    fn operate(
        &mut self,
        layout: layout::Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        self.content
            .as_widget_mut()
            .operate(self.tree, layout, renderer, operation);
    }

    fn update(
        &mut self,
        event: &Event,
        layout: layout::Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
    ) {
        let is_over = cursor.is_over(layout.bounds());

        // Close when clicking outside the popup content.
        if let Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) = event
            && !is_over
        {
            if let Some(message) = self.on_close.clone() {
                shell.publish(message);
            }

            //shell.capture_event();
            return;
        }

        self.content.as_widget_mut().update(
            self.tree,
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            &layout.bounds(),
        );

        if is_over {
            shell.capture_event();
        }
    }

    fn mouse_interaction(
        &self,
        layout: layout::Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        if !cursor.is_over(layout.bounds()) {
            return mouse::Interaction::default();
        }

        // Find out if any of our widgets would like a specific cursor
        let interaction = self.content.as_widget().mouse_interaction(
            self.tree,
            layout,
            cursor,
            &layout.bounds(),
            renderer,
        );

        if interaction == mouse::Interaction::default() {
            // Force to idle, to prevent widgets under us from changing it
            mouse::Interaction::Idle
        } else {
            interaction
        }
    }
}

impl<'a, Message, Theme, Renderer> From<Popover<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a + Clone,
    Theme: 'a,
    Renderer: 'a + renderer::Renderer,
{
    fn from(popover: Popover<'a, Message, Theme, Renderer>) -> Self {
        Element::new(popover)
    }
}
