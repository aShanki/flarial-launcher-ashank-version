using System;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Media;
using Avalonia.Threading;
using Avalonia.VisualTree;
using Flarial.Runtime.Services;

namespace Flarial.Launcher.Controls;

internal sealed class SponsorBannerImage(string sponsorId, string campaignId) : Control
{
    public IImage? Source { get; init; }
    bool _recorded;
    int _attachment;
    Window? _window;

    protected override void OnAttachedToVisualTree(VisualTreeAttachmentEventArgs e)
    {
        base.OnAttachedToVisualTree(e);
        _recorded = false;
        _attachment++;
        _window = TopLevel.GetTopLevel(this) as Window;
        if (_window is { }) _window.Activated += OnActivated;
    }

    protected override void OnDetachedFromVisualTree(VisualTreeAttachmentEventArgs e)
    {
        if (_window is { }) _window.Activated -= OnActivated;
        _window = null;
        _attachment++;
        base.OnDetachedFromVisualTree(e);
    }

    void OnActivated(object? sender, EventArgs e) => InvalidateVisual();

    public override void Render(DrawingContext context)
    {
        if (Source is { } source && source.Size.Width > 0 && source.Size.Height > 0)
        {
            var scale = Math.Min(Bounds.Width / source.Size.Width, Bounds.Height / source.Size.Height);
            var size = new Size(source.Size.Width * scale, source.Size.Height * scale);
            var origin = new Point((Bounds.Width - size.Width) / 2, (Bounds.Height - size.Height) / 2);
            context.DrawImage(source, new Rect(source.Size), new Rect(origin, size));
        }
        if (_recorded || Source is null || Source.Size.Width <= 0 || Source.Size.Height <= 0 || !CanCount()) return;
        var attachment = _attachment;
        Dispatcher.UIThread.Post(() =>
        {
            if (_recorded || attachment != _attachment || !CanCount()) return;
            _recorded = true;
            SponsorAnalyticsService.Record(sponsorId, campaignId, "impression");
        }, DispatcherPriority.Background);
    }

    bool CanCount()
    {
        if (_window is not { IsVisible: true, IsActive: true } window || window.WindowState == WindowState.Minimized ||
            Bounds.Width <= 0 || Bounds.Height <= 0) return false;
        for (Visual? visual = this; visual is { }; visual = visual.GetVisualParent())
        {
            if (!visual.IsVisible || visual.Opacity <= 0) return false;
            var position = this.TranslatePoint(default, visual);
            if (position is null) return false;
            if (visual.ClipToBounds && !new Rect(visual.Bounds.Size).Intersects(new Rect(position.Value, Bounds.Size))) return false;
        }
        var origin = this.TranslatePoint(default, window);
        return origin is { } point && new Rect(window.ClientSize).Intersects(new Rect(point, Bounds.Size));
    }

    public void RecordClick() => SponsorAnalyticsService.Record(sponsorId, campaignId, "click");
}
