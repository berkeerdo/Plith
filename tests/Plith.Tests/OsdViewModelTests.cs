using Plith.ViewModels;

namespace Plith.Tests;

public class OsdViewModelTests
{
    [Fact]
    public void ShowMediaCard_TrueWhenSessionAndNotCompact()
    {
        var vm = new OsdViewModel();
        vm.Media.HasSession = true;
        vm.CompactMode = false;
        Assert.True(vm.ShowMediaCard);
    }

    [Fact]
    public void ShowMediaCard_FalseWhenCompact()
    {
        var vm = new OsdViewModel();
        vm.Media.HasSession = true;
        vm.CompactMode = true;
        Assert.False(vm.ShowMediaCard);
    }

    [Fact]
    public void ShowMediaCard_FalseWhenNoSession()
    {
        var vm = new OsdViewModel();
        vm.Media.HasSession = false;
        vm.CompactMode = false;
        Assert.False(vm.ShowMediaCard);
    }
}
