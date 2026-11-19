// SPDX-License-Identifier: Apache-2.0
//! SigmaOS Notification Center Plugin for Omarchy & Mint Personas.
//!
//! Implements contracts: `notification-center-plugin`, `history-drawer`, `dnd-toggle`.
//! Features:
//! - Notification history ring buffer with dismissal and filtering
//! - Do-Not-Disturb (DND) mode with critical urgency bypass
//! - App categorization and unread counters
//! - Plugin interface integration for Waybar & Quickshell status bars

#![forbid(unsafe_code)]

use std::fmt;

/// Notification urgency level (Desktop Notifications Specification).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Urgency {
    Low = 0,
    #[default]
    Normal = 1,
    Critical = 2,
}

/// A desktop notification item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notification {
    pub id: u64,
    pub app_name: String,
    pub summary: String,
    pub body: String,
    pub urgency: Urgency,
    pub timestamp_sec: u64,
    pub read: bool,
}

/// Errors for notification operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotificationError {
    EmptySummary,
    NotFound(u64),
}

impl fmt::Display for NotificationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptySummary => write!(f, "notification summary must not be empty"),
            Self::NotFound(id) => write!(f, "notification with ID {id} not found"),
        }
    }
}

impl std::error::Error for NotificationError {}

/// History drawer and notification manager with DND support.
#[derive(Debug, Clone)]
pub struct NotificationCenter {
    notifications: Vec<Notification>,
    max_history: usize,
    next_id: u64,
    dnd_active: bool,
}

impl Default for NotificationCenter {
    fn default() -> Self {
        Self::new(100)
    }
}

impl NotificationCenter {
    pub fn new(max_history: usize) -> Self {
        Self {
            notifications: Vec::new(),
            max_history: if max_history == 0 { 50 } else { max_history },
            next_id: 1,
            dnd_active: false,
        }
    }

    /// Toggle or set Do Not Disturb mode.
    pub fn set_dnd(&mut self, active: bool) {
        self.dnd_active = active;
    }

    pub fn is_dnd_active(&self) -> bool {
        self.dnd_active
    }

    /// Post a new notification. Returns the assigned ID and whether an immediate popup was shown.
    /// In DND mode, only Critical notifications display immediately; non-critical are filed directly into history.
    pub fn post(
        &mut self,
        app_name: impl Into<String>,
        summary: impl Into<String>,
        body: impl Into<String>,
        urgency: Urgency,
        timestamp_sec: u64,
    ) -> Result<(u64, bool), NotificationError> {
        let summary_str = summary.into();
        if summary_str.trim().is_empty() {
            return Err(NotificationError::EmptySummary);
        }

        let id = self.next_id;
        self.next_id += 1;

        let shown_immediately = !self.dnd_active || urgency == Urgency::Critical;

        let notif = Notification {
            id,
            app_name: app_name.into(),
            summary: summary_str,
            body: body.into(),
            urgency,
            timestamp_sec,
            read: false,
        };

        self.notifications.push(notif);

        // Enforce maximum history capacity
        if self.notifications.len() > self.max_history {
            self.notifications.remove(0);
        }

        Ok((id, shown_immediately))
    }

    /// Mark a notification as read.
    pub fn mark_read(&mut self, id: u64) -> Result<(), NotificationError> {
        let item = self.notifications.iter_mut().find(|n| n.id == id)
            .ok_or(NotificationError::NotFound(id))?;
        item.read = true;
        Ok(())
    }

    /// Dismiss / remove a single notification from history.
    pub fn dismiss(&mut self, id: u64) -> Result<(), NotificationError> {
        let pos = self.notifications.iter().position(|n| n.id == id)
            .ok_or(NotificationError::NotFound(id))?;
        self.notifications.remove(pos);
        Ok(())
    }

    /// Clear all notifications in history.
    pub fn clear_all(&mut self) {
        self.notifications.clear();
    }

    /// Get total notifications in history.
    pub fn len(&self) -> usize {
        self.notifications.len()
    }

    pub fn is_empty(&self) -> bool {
        self.notifications.is_empty()
    }

    /// Count unread notifications.
    pub fn unread_count(&self) -> usize {
        self.notifications.iter().filter(|n| !n.read).count()
    }

    /// View all history items (newest last).
    pub fn history(&self) -> &[Notification] {
        &self.notifications
    }
}

/// Errors for `PluginError` operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginError {
    Duplicate(String),
    NotFound(String),
    EmptyName,
    ExecFailed(String),
}

impl fmt::Display for PluginError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Duplicate(x) => write!(f, "plugin name already registered: {x}"),
            Self::NotFound(x) => write!(f, "plugin not registered: {x}"),
            Self::EmptyName => write!(f, "plugin name must not be empty"),
            Self::ExecFailed(x) => write!(f, "plugin execution failed: {x}"),
        }
    }
}

impl std::error::Error for PluginError {}

/// A plugin contributes a named action to the host.
pub trait Plugin {
    fn name(&self) -> &str;
    fn execute(&self) -> Result<String, PluginError>;
}

/// Notification Center plugin implementation.
pub struct NotificationCenterPlugin {
    pub center: NotificationCenter,
}

impl Default for NotificationCenterPlugin {
    fn default() -> Self {
        Self {
            center: NotificationCenter::default(),
        }
    }
}

impl Plugin for NotificationCenterPlugin {
    fn name(&self) -> &str {
        "notification-center"
    }

    fn execute(&self) -> Result<String, PluginError> {
        let unread = self.center.unread_count();
        let total = self.center.len();
        let dnd = if self.center.is_dnd_active() { "DND: ON" } else { "DND: OFF" };
        Ok(format!("Notifications: {unread} unread / {total} total [{dnd}]"))
    }
}

/// Registry of named plugins.
#[derive(Default)]
pub struct PluginRegistry {
    plugins: Vec<Box<dyn Plugin>>,
}

impl PluginRegistry {
    pub fn new() -> Self { Self::default() }

    pub fn register(&mut self, plugin: Box<dyn Plugin>) -> Result<(), PluginError> {
        let n = plugin.name();
        if n.trim().is_empty() { return Err(PluginError::EmptyName); }
        if self.plugins.iter().any(|p| p.name() == n) {
            return Err(PluginError::Duplicate(n.to_string()));
        }
        self.plugins.push(plugin);
        Ok(())
    }

    pub fn run(&self, name: &str) -> Result<String, PluginError> {
        self.plugins.iter().find(|p| p.name() == name)
            .ok_or_else(|| PluginError::NotFound(name.to_string()))?
            .execute()
    }

    pub fn names(&self) -> Vec<&str> {
        self.plugins.iter().map(|p| p.name()).collect()
    }

    pub fn len(&self) -> usize { self.plugins.len() }
    pub fn is_empty(&self) -> bool { self.plugins.is_empty() }
}

/// Backwards-compatible component handle.
#[derive(Debug, Default)]
pub struct Component {
    name: &'static str,
}

impl Component {
    pub fn new() -> Self {
        Self { name: "SigmaOS Notification Center" }
    }

    pub fn name(&self) -> &'static str {
        self.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notification_post_and_unread() {
        let mut center = NotificationCenter::new(10);
        let (id1, shown1) = center.post("slack", "New message", "Hello", Urgency::Normal, 100).unwrap();
        assert!(shown1);
        assert_eq!(center.unread_count(), 1);

        center.mark_read(id1).unwrap();
        assert_eq!(center.unread_count(), 0);
    }

    #[test]
    fn test_dnd_mode_suppression_and_bypass() {
        let mut center = NotificationCenter::new(10);
        center.set_dnd(true);
        assert!(center.is_dnd_active());

        // Normal urgency is filed quietly without immediate popup
        let (_, shown_normal) = center.post("mail", "Newsletter", "Discount inside", Urgency::Normal, 200).unwrap();
        assert!(!shown_normal);

        // Critical urgency bypasses DND mode
        let (_, shown_crit) = center.post("power", "Battery 5%", "Plug in immediately", Urgency::Critical, 201).unwrap();
        assert!(shown_crit);
        assert_eq!(center.len(), 2);
    }

    #[test]
    fn test_max_history_limit() {
        let mut center = NotificationCenter::new(3);
        center.post("app", "msg 1", "body", Urgency::Low, 1).unwrap();
        center.post("app", "msg 2", "body", Urgency::Low, 2).unwrap();
        center.post("app", "msg 3", "body", Urgency::Low, 3).unwrap();
        center.post("app", "msg 4", "body", Urgency::Low, 4).unwrap();

        assert_eq!(center.len(), 3);
        assert_eq!(center.history()[0].summary, "msg 2");
    }

    #[test]
    fn test_plugin_interface() {
        let mut plugin = NotificationCenterPlugin::default();
        plugin.center.post("app", "Alert", "Text", Urgency::Normal, 10).unwrap();
        assert_eq!(plugin.name(), "notification-center");
        let output = plugin.execute().unwrap();
        assert!(output.contains("1 unread"));
    }
}
