using System.Windows;
using System.Windows.Controls;

namespace Plith.Views.Presentation;

/// <summary>
/// A <see cref="TextBlock"/> whose text is upper-cased while the <c>OrnamentLabelCase</c> token
/// says <c>"Upper"</c>.
/// </summary>
/// <remarks>
/// WPF has no text-transform, and the label text is mostly bound, so a converter cannot see the
/// token and a code-behind write would miss every later binding update. Coercing
/// <see cref="TextBlock.TextProperty"/> handles both: the stored value stays as bound, and what
/// is displayed is derived from it. Changing <see cref="Case"/> re-coerces, so a theme swap that
/// changes the token (high contrast turning it off) reaches a label that is already on screen.
/// </remarks>
public class LoftLabel : TextBlock
{
    public static readonly DependencyProperty CaseProperty = DependencyProperty.Register(
        nameof(Case), typeof(string), typeof(LoftLabel),
        new PropertyMetadata("None", static (d, _) => d.CoerceValue(TextProperty)));

    static LoftLabel()
    {
        TextProperty.OverrideMetadata(
            typeof(LoftLabel),
            new FrameworkPropertyMetadata(string.Empty, null, CoerceText));
    }

    /// <summary>"Upper" or "None". Anything else is treated as "None".</summary>
    public string Case
    {
        get => (string)GetValue(CaseProperty);
        set => SetValue(CaseProperty, value);
    }

    private static object CoerceText(DependencyObject d, object value) =>
        d is LoftLabel { Case: "Upper" } && value is string text
            ? text.ToUpperInvariant()
            : value;
}
