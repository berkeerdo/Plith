using System.Windows.Controls;
using Plith.ViewModels;

namespace Plith.Views;

public partial class AudioCardView : UserControl
{
    public AudioCardView()
    {
        InitializeComponent();
        LiveRegionAnnouncer.Attach(this, nameof(AudioCardViewModel.AccessibleSummary));
    }
}
