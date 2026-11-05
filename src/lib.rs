#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationMessage {
    pub app_id: String,
    pub summary: String,
    pub body: String,
    pub read: bool,
}

#[derive(Debug, Default)]
pub struct NotificationCenterPlugin {
    notifications: Vec<NotificationMessage>,
    pub do_not_disturb: bool,
}

impl NotificationCenterPlugin {
    pub fn new() -> Self { Self { notifications: Vec::new(), do_not_disturb: false } }
    pub fn push(&mut self, app: &str, sum: &str, body: &str) {
        if !self.do_not_disturb {
            self.notifications.push(NotificationMessage {
                app_id: app.into(),
                summary: sum.into(),
                body: body.into(),
                read: false,
            });
        }
    }
    pub fn unread_count(&self) -> usize {
        self.notifications.iter().filter(|n| !n.read).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_notification_center_plugin_aaa() {
        // Arrange
        let mut plugin = NotificationCenterPlugin::new();
        // Act
        plugin.push("Mail", "New Email", "Subject: Release v22");
        assert_eq!(plugin.unread_count(), 1);
        plugin.do_not_disturb = true;
        plugin.push("Chat", "Ping", "Are you available?");
        // Assert: DND ignored the second message
        assert_eq!(plugin.unread_count(), 1);
    }
}
