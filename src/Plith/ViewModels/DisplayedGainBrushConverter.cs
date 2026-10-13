using System.Globalization;
using System.Windows.Data;
using System.Windows.Media;

namespace Plith.ViewModels;

/// <summary>
/// Picks the gain colour for the level the bar is showing rather than the level it is heading
/// for. Inputs: the displayed level, the view model, and <c>GainColor</c> (bound only so the
/// result refreshes when the theme, mute or threshold mode changes). With the parameter
/// <c>Color</c> it returns the brush's colour, for the glow.
/// </summary>
public sealed class DisplayedGainBrushConverter : IMultiValueConverter
{
    public object Convert(object[] values, Type targetType, object? parameter, CultureInfo culture)
    {
        var level = values.Length > 0 && values[0] is double d ? d : 0d;
        var brush = values.Length > 1 && values[1] is AudioCardViewModel vm
            ? vm.GainColorAt(level)
            : Brushes.Transparent;

        if (parameter as string == "Color")
        {
            return brush is SolidColorBrush solid ? solid.Color : Colors.Transparent;
        }

        return brush;
    }

    public object[] ConvertBack(object value, Type[] targetTypes, object? parameter, CultureInfo culture)
        => throw new NotSupportedException();
}
