using System.Windows.Automation.Peers;
using System.Windows.Controls;

namespace Plith.Views.Presentation;

/// <summary>
/// Text that is ornament, not content: it creates no automation peer, so it never reaches the
/// accessibility tree.
/// </summary>
/// <remarks>
/// WPF has no equivalent of UWP's <c>AutomationProperties.AccessibilityView="Raw"</c>. A
/// <see cref="TextBlock"/> always surfaces as a Text element, and a screen reader would read the
/// output picker's brackets as "left bracket". Declining to create the peer is the WPF way to
/// leave an element out entirely; the owning control's own name carries the meaning.
/// </remarks>
public sealed class DecorativeText : TextBlock
{
    protected override AutomationPeer? OnCreateAutomationPeer() => null;
}
