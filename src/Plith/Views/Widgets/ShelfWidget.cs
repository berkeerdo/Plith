using System.Windows;
using System.Windows.Controls;
using System.Windows.Threading;
using Plith.Services.Shelf;

namespace Plith.Views.Widgets;

/// <summary>
/// The shelf page, which draws nothing but the reason the shelf could not open.
///
/// THIS FILE USED TO BE THE SHELF. It drew five tiles, a "+N" overflow chip, an empty state, a
/// line saying how to open the surface, and it extracted shell icons to fill the tiles: 552 lines
/// of a second shelf. All of it is deleted, and what replaced it is not a smaller version of the
/// same thing, it is the acknowledgement that there is only one shelf.
///
/// The shelf lives in Plith.DropCatcher, because a file can only be dragged out of that process's
/// window: Plith runs at high integrity in a Release build and DoDragDrop carries nothing from
/// there, and a press cannot be delegated between processes. Both measured, in
/// docs/SHELF-VERIFICATION.md section 4. Since the handover happens on the page turn
/// (OsdHost.ReconcileShelfFrame), the catcher's window covers this page about 25 ms after it
/// arrives, so everything this page drew was a different-looking imitation of the page about to
/// replace it. A person reported it three times before the answer was to delete it: as the design
/// being bad, as the drop moment not being smooth, and finally as "why are there two shelves".
///
/// What is left is the one thing the catcher cannot say. When it cannot be reached the handover
/// never happens, nothing covers this page, and a blank panel tells a person nothing at all:
/// <see cref="ShowUnavailable"/> puts the reason here instead. That sentence is why this control
/// still exists rather than being removed from the pager.
///
/// It also no longer watches ShelfStore. It has nothing to repaint, so a page that subscribed
/// would be a page doing work for a surface that is never on screen.
/// </summary>
public partial class ShelfWidget : UserControl
{
    /// <summary>How long a failure sentence stays before the page goes blank again. Long enough
    /// to read, short enough that a person who fixes the install does not have to wait it
    /// out.</summary>
    private static readonly TimeSpan UnavailableFor = TimeSpan.FromSeconds(6);

    private DispatcherTimer? _unavailable;

    /// <summary>
    /// The store is still taken, and still not used, on purpose.
    ///
    /// Keeping it in the signature costs nothing and keeps the page's identity: this is the shelf's
    /// page, constructed by whoever owns the shelf. Dropping the parameter would make the page
    /// constructible from anywhere, and the next hand to need "just a blank page" would find one.
    /// </summary>
    public ShelfWidget(ShelfStore shelf)
    {
        InitializeComponent();
        _ = shelf;
    }

    /// <summary>
    /// Nothing to unsubscribe from any more, and the method stays: CardHost calls it for every
    /// page it discards, and a page that silently stopped needing it would leave the next one to
    /// discover whether the call is still made.
    /// </summary>
    public void Detach() => _unavailable?.Stop();

    /// <summary>
    /// Say why the shelf did not open.
    ///
    /// The one thing this page draws, and the only reason it is not deleted outright. A click or
    /// a page turn that produces nothing is indistinguishable from the product being broken, and
    /// this is the one interaction whose failure is otherwise entirely invisible: the helper
    /// process is not something anyone knows exists.
    /// </summary>
    public void ShowUnavailable(string why)
    {
        Hint.Text = why;
        Hint.Visibility = Visibility.Visible;

        if (_unavailable is null)
        {
            _unavailable = new DispatcherTimer(DispatcherPriority.Normal, Dispatcher) { Interval = UnavailableFor };
            _unavailable.Tick += (_, _) =>
            {
                _unavailable!.Stop();
                Hint.Visibility = Visibility.Collapsed;
            };
        }

        // Stopped before started, so a second failure restarts the clock rather than leaving the
        // sentence to disappear on the first one's schedule.
        _unavailable.Stop();
        _unavailable.Start();
    }
}
