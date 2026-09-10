namespace Plith.Services;

/// <summary>
/// Conversions between the normalised 0..1 a slider works in and what each audio source
/// actually wants.
///
/// Pure, and separate from both clients, because this is the one part of the write path that
/// can be tested at all: the COM call and the P/Invoke need a real endpoint and a running
/// Voicemeeter, while getting the arithmetic wrong is what makes a drag land somewhere other
/// than where the finger was.
/// </summary>
public static class VolumeMath
{
    /// <summary>Voicemeeter's gain range, in dB. The same numbers the view model already uses
    /// to draw the bar — kept in one place so the drawn position and the written value cannot
    /// disagree about what "half way" means.</summary>
    public const float VoicemeeterMinDb = VoicemeeterClient.MinGainDb;
    public const float VoicemeeterMaxDb = VoicemeeterClient.MaxGainDb;

    public static double Clamp01(double t) => t < 0 ? 0 : t > 1 ? 1 : t;

    /// <summary>
    /// Normalised position to a Voicemeeter gain.
    ///
    /// Linear in dB, which is the same mapping the bar is drawn with. It is not the mapping a
    /// perceptual fader would use — most of the audible range sits in the top third — but it
    /// matches what Voicemeeter's own faders do, and a slider in the notch that disagreed with
    /// the slider in Voicemeeter would be worse than one that is merely not perceptual.
    /// </summary>
    public static float NormalizedToVoicemeeterDb(double normalized) =>
        (float)(VoicemeeterMinDb + (VoicemeeterMaxDb - VoicemeeterMinDb) * Clamp01(normalized));

    /// <summary>The inverse. Values outside Voicemeeter's range clamp rather than extrapolating:
    /// a bus set from Voicemeeter's own UI can sit outside it, and a bar drawn past its own ends
    /// is worse than one pinned at them.</summary>
    public static double VoicemeeterDbToNormalized(float gainDb) =>
        Clamp01((gainDb - VoicemeeterMinDb) / (VoicemeeterMaxDb - VoicemeeterMinDb));

    /// <summary>
    /// Normalised position to a Windows endpoint scalar.
    ///
    /// Identity, deliberately, rather than a curve. <c>MasterVolumeLevelScalar</c> is already
    /// the value Windows' own volume slider sits at, so a curve here would put Plith's slider
    /// somewhere other than the system one for the same audible level.
    /// </summary>
    public static float NormalizedToWindowsScalar(double normalized) => (float)Clamp01(normalized);

    /// <summary>
    /// Snap a drag position to a step, so a drag lands on round numbers rather than 47.318 %.
    ///
    /// <paramref name="stepPercent"/> of 0 or less means no snapping — the caller asked for the
    /// raw position and gets it, instead of a division by zero.
    /// </summary>
    public static double SnapToStep(double normalized, double stepPercent)
    {
        var t = Clamp01(normalized);
        if (stepPercent <= 0) return t;

        var step = stepPercent / 100.0;
        return Clamp01(Math.Round(t / step, MidpointRounding.AwayFromZero) * step);
    }

    /// <summary>
    /// Where along a track a pointer at <paramref name="x"/> is, 0..1.
    ///
    /// A zero or negative width returns 0 rather than dividing: a track is measured during
    /// layout, and a drag can begin on the frame's very first arrange pass, when the width has
    /// not been assigned yet.
    /// </summary>
    public static double PositionOnTrack(double x, double trackWidth) =>
        trackWidth <= 0 ? 0 : Clamp01(x / trackWidth);
}
