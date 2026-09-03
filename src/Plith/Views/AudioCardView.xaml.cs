using System.Windows.Controls;

namespace Plith.Views;

public partial class AudioCardView : UserControl
{
    public AudioCardView()
    {
        InitializeComponent();
        LiveRegionAnnouncer.Attach(this, nameof(ViewModels.AudioCardViewModel.AccessibleSummary));
    }
}
