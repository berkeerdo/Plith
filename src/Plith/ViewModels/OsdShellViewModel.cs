using System.Collections.ObjectModel;
using Plith.Cards;

namespace Plith.ViewModels;

/// <summary>
/// Binding root for OsdContent. Phase 5 needs nothing beyond the visible-card list — the
/// collection instance comes straight from CardHost, so no change-forwarding is needed.
/// Shell-level state (notch height, preset mode) arrives in Phase 6.
/// </summary>
public sealed class OsdShellViewModel
{
    public OsdShellViewModel(CardHost host) => VisibleCards = host.VisibleCards;

    public ObservableCollection<ICard> VisibleCards { get; }
}
