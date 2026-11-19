// Integration test for SigmaOS-notification-center-plugin
use sigmaos_notification_center_plugin::{NotificationCenter, NotificationCenterPlugin, Urgency, PluginRegistry, Component};

#[test]
fn test_notification_center_full_lifecycle() {
    let mut center = NotificationCenter::new(50);
    let (id, shown) = center.post("terminal", "Build succeeded", "Target x86_64 ready", Urgency::Normal, 1000).unwrap();
    assert!(shown);
    assert_eq!(center.unread_count(), 1);

    center.mark_read(id).unwrap();
    assert_eq!(center.unread_count(), 0);

    center.dismiss(id).unwrap();
    assert_eq!(center.len(), 0);
}

#[test]
fn test_plugin_registry_integration() {
    let mut registry = PluginRegistry::new();
    let plugin = NotificationCenterPlugin::default();
    registry.register(Box::new(plugin)).unwrap();

    let res = registry.run("notification-center").unwrap();
    assert!(res.contains("Notifications: 0 unread"));
}

#[test]
fn test_component_handle() {
    let comp = Component::new();
    assert_eq!(comp.name(), "SigmaOS Notification Center");
}
