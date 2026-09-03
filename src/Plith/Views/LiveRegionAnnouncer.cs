using System.ComponentModel;
using System.Windows;
using System.Windows.Automation.Peers;

namespace Plith.Views;

/// <summary>
/// Keeps a card view's UI Automation live region announcing. Two WPF details make this
/// necessary, and both were measured against the running app's UIA tree rather than assumed:
///
///  * <c>AutomationProperties</c> only reach UI Automation through an element that owns an
///    automation peer — <c>UIElementAutomationPeer.GetNameCore</c> reads
///    <c>AutomationProperties.GetName(owner)</c>. WPF creates no peer for a bare
///    <see cref="System.Windows.Controls.Grid"/> or other panel, so a name set there is
///    silently dropped: it appears nowhere in the tree, not even in the raw view. That is why
///    the card views carry these properties on their <c>UserControl</c> root, which does own a
///    peer. Moving them back onto an inner panel would disable screen-reader support without
///    breaking a build or a test.
///
///  * WPF does not raise <see cref="AutomationEvents.LiveRegionChanged"/> on its own when the
///    bound name changes. Without the explicit raise below a screen reader keeps whatever value
///    it read when the OSD first appeared, which for the volume card is every value but the
///    current one.
/// </summary>
internal static class LiveRegionAnnouncer
{
    /// <summary>
    /// Raises <see cref="AutomationEvents.LiveRegionChanged"/> on <paramref name="view"/>
    /// whenever <paramref name="summaryPropertyName"/> changes on its DataContext.
    /// </summary>
    public static void Attach(FrameworkElement view, string summaryPropertyName)
    {
        INotifyPropertyChanged? source = null;

        void OnSourcePropertyChanged(object? sender, PropertyChangedEventArgs e)
        {
            // An empty or null PropertyName is WPF's "everything changed" signal, so it counts.
            if (!string.IsNullOrEmpty(e.PropertyName) && e.PropertyName != summaryPropertyName)
                return;

            // FromElement returns the existing peer and never creates one. Null means no UI
            // Automation client has connected, which is exactly when there is nobody to
            // announce to — building a peer just to raise into the void would be waste.
            UIElementAutomationPeer.FromElement(view)
                ?.RaiseAutomationEvent(AutomationEvents.LiveRegionChanged);
        }

        void Rebind(object? newContext)
        {
            if (source is not null) source.PropertyChanged -= OnSourcePropertyChanged;
            source = newContext as INotifyPropertyChanged;
            if (source is not null) source.PropertyChanged += OnSourcePropertyChanged;
        }

        // A card's container is recreated whenever CardHost inserts it back into VisibleCards,
        // so a view can be constructed before its DataContext arrives and can outlive one.
        Rebind(view.DataContext);
        view.DataContextChanged += (_, e) => Rebind(e.NewValue);

        // Loaded and Unloaded must be symmetric. Unloaded alone drops the subscription so a
        // discarded view stops holding the long-lived view-model, but if the same instance is
        // ever put back into the tree with the DataContext it already had, DataContextChanged
        // does not fire — and without the Loaded handler the announcer would stay silently
        // dead from then on. That is the same failure shape as the defect this class fixes:
        // correct-looking markup that quietly announces nothing.
        view.Loaded += (_, _) => Rebind(view.DataContext);
        view.Unloaded += (_, _) => Rebind(null);
    }
}
