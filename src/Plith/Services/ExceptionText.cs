namespace Plith.Services;

/// <summary>
/// Renders an exception for a diagnostic log line.
///
/// Exists because the type name alone is not diagnosable, and that was measured rather than
/// guessed. The first live run of the weather slice logged
/// <c>Current-conditions fetch failed: HttpRequestException</c> while the very same URL
/// returned HTTP 200 from the same machine seconds later — so the failure was inside the app,
/// and the line as written could not say which of DNS, TLS, a proxy, a refused connection or a
/// malformed request it was. Every one of those reasons lives in the message or the inner
/// exception, neither of which was being recorded.
///
/// Used by the paths that degrade silently on failure — the weather fetch and both location
/// providers. Those deliberately show the user nothing when they fail, which makes the log the
/// only evidence that exists, so it has to carry enough to act on.
/// </summary>
internal static class ExceptionText
{
    /// <summary>
    /// Type name plus message, and the inner exception's too when there is one. HttpRequestException
    /// in particular keeps the useful cause (a SocketException, an AuthenticationException) inside
    /// InnerException while its own message stays generic.
    ///
    /// This does NOT weaken the callers' once-per-transition logging: they dedup on the whole
    /// composed string, so a repeated identical failure still logs exactly once. It only makes the
    /// signature finer-grained, which is the right trade for a path whose only symptom is silence.
    /// </summary>
    public static string Describe(Exception ex)
        => ex.InnerException is { } inner
            ? $"{ex.GetType().Name}: {ex.Message} -> {inner.GetType().Name}: {inner.Message}"
            : $"{ex.GetType().Name}: {ex.Message}";
}
