import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Loader2, AlertTriangle, Sparkles } from "lucide-react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { commands, type AppError, type Promotion } from "@/ipc/bindings";
import { cn } from "@/lib/cn";

function errorMessage(err: unknown): string {
  if (err && typeof err === "object" && "kind" in err) {
    const e = err as AppError;
    return e.detail ? `${e.kind}: ${e.detail}` : e.kind;
  }
  return String(err);
}

// `host` is what we hand to Google's favicon CDN for the icon — pick the
// canonical homepage of each server. `color` is the gradient shown while the
// favicon loads and as a fallback if it 404s. `label` is the letter fallback.
const SERVERS: { id: string; label: string; color: string; host: string; title: string }[] = [
  { id: "hypixel", label: "H", host: "hypixel.net", title: "Hypixel", color: "from-yellow-500 to-amber-700" },
  { id: "cubecraft", label: "C", host: "cubecraft.net", title: "CubeCraft", color: "from-blue-500 to-indigo-700" },
  { id: "lifeboat", label: "L", host: "lbsg.net", title: "Lifeboat", color: "from-rose-500 to-rose-800" },
  { id: "mineplex", label: "M", host: "mineplex.com", title: "Mineplex", color: "from-purple-500 to-purple-800" },
  { id: "galaxite", label: "G", host: "galaxite.net", title: "Galaxite", color: "from-fuchsia-500 to-pink-700" },
  { id: "pgg", label: "P", host: "playpgg.com", title: "PGG", color: "from-emerald-500 to-emerald-800" },
  { id: "nethergames", label: "N", host: "nethergames.org", title: "NetherGames", color: "from-orange-500 to-red-700" },
  { id: "complex", label: "X", host: "hcgaming.net", title: "Complex Gaming", color: "from-cyan-500 to-sky-800" },
];

function faviconUrl(host: string): string {
  return `https://www.google.com/s2/favicons?domain=${encodeURIComponent(host)}&sz=64`;
}

export function HomePage() {
  const qc = useQueryClient();

  const pkg = useQuery({
    queryKey: ["minecraft", "package"],
    queryFn: commands.getMinecraftPackage,
  });

  const promotions = useQuery({
    queryKey: ["promotions"],
    queryFn: commands.listPromotions,
    staleTime: 30 * 60 * 1000,
    gcTime: 24 * 60 * 60 * 1000,
  });

  const launch = useMutation({
    mutationFn: commands.launchMinecraft,
    onSettled: () => qc.invalidateQueries({ queryKey: ["minecraft"] }),
  });

  const installed = pkg.data?.installed;
  const ready = installed && pkg.data?.gaming_services_installed;

  const blocked = installed && !pkg.data?.gaming_services_installed
    ? "Gaming Services missing — install from Microsoft Store."
    : !installed && !pkg.isLoading
      ? "Minecraft for Windows is not installed."
      : null;

  return (
    <div className="flex h-screen flex-col gap-3 px-6 py-5">
      <HeroPanel
        version={pkg.data?.version ?? null}
        ready={!!ready}
        launching={launch.isPending}
        onLaunch={() => launch.mutate()}
        promotion={promotions.data?.[0] ?? null}
      />

      <ServerRow />

      <ResultLine
        success={launch.isSuccess ? `Injected · pid ${launch.data}` : null}
        error={launch.isError ? errorMessage(launch.error) : null}
        blocked={blocked}
      />
    </div>
  );
}

function HeroPanel({
  version,
  ready,
  launching,
  onLaunch,
  promotion,
}: {
  version: string | null;
  ready: boolean;
  launching: boolean;
  onLaunch: () => void;
  promotion: Promotion | null;
}) {
  return (
    <div className="relative flex-1 min-h-[200px] overflow-hidden rounded-[var(--radius)] border border-border">
      <div
        className="absolute inset-0 bg-cover bg-center"
        style={{
          backgroundImage: 'url("/hero-bg.webp")',
          filter: "saturate(0.85) contrast(1.05) brightness(0.85)",
        }}
        aria-hidden
      />
      <div
        className="absolute inset-0"
        style={{
          background:
            "linear-gradient(rgba(18,14,15,0.35) 0%, rgba(18,14,15,0.55) 45%, rgba(18,14,15,0.92) 85%, rgb(18,14,15) 100%)",
        }}
        aria-hidden
      />
      <div
        className="absolute inset-0"
        style={{
          background:
            "radial-gradient(95% 70% at 50% 45%, transparent 30%, rgba(18,14,15,0.75) 100%)",
        }}
        aria-hidden
      />

      <div className="relative flex h-full items-center justify-center px-6">
        <LaunchControl
          version={version}
          ready={ready}
          launching={launching}
          onLaunch={onLaunch}
        />
      </div>

      {promotion && <PromotionOverlay promotion={promotion} />}
    </div>
  );
}

function PromotionOverlay({ promotion }: { promotion: Promotion }) {
  return (
    <button
      type="button"
      onClick={() => openUrl(promotion.Uri).catch(() => {})}
      title="Sponsored"
      className="group absolute left-1/2 top-4 z-10 -translate-x-1/2 cursor-pointer transition-transform hover:scale-[1.03]"
    >
      <img
        src={promotion.Image}
        alt="Sponsor"
        className="block h-16 w-auto drop-shadow-[0_4px_18px_rgba(0,0,0,0.55)]"
        draggable={false}
        referrerPolicy="no-referrer"
      />
    </button>
  );
}

function LaunchControl({
  version,
  ready,
  launching,
  onLaunch,
}: {
  version: string | null;
  ready: boolean;
  launching: boolean;
  onLaunch: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onLaunch}
      disabled={!ready || launching}
      className={cn(
        "group relative w-[360px] overflow-hidden rounded-xl px-10 py-4 text-center",
        "bg-accent text-foreground transition-transform active:scale-[0.98]",
        "ring-1 ring-inset ring-foreground/20",
        "disabled:cursor-not-allowed disabled:bg-elevated disabled:text-faint",
      )}
      style={{
        background:
          "linear-gradient(180deg, oklch(0.72 0.24 22) 0%, oklch(0.6 0.24 22) 100%)",
      }}
    >
      <span
        aria-hidden
        className="pointer-events-none absolute inset-x-0 top-0 h-px bg-foreground/45"
      />
      <span
        aria-hidden
        className="pointer-events-none absolute inset-x-0 bottom-0 h-px bg-black/30"
      />

      <span className="relative block">
        <span className="font-display block text-3xl font-extrabold uppercase tracking-[-0.01em] leading-none text-foreground drop-shadow-[0_1px_0_rgba(0,0,0,0.3)]">
          {launching ? (
            <span className="inline-flex items-center gap-3">
              <Loader2 className="size-6 animate-spin" /> LAUNCHING
            </span>
          ) : (
            "LAUNCH GAME"
          )}
        </span>
        <span className="font-sans mt-1.5 block text-[13px] font-medium text-foreground/90">
          Flarial Client {version ?? "—"}
        </span>
      </span>
    </button>
  );
}

function ServerRow() {
  return (
    <div className="flex items-center gap-2 overflow-x-auto rounded-[var(--radius-sm)] border border-border bg-elevated/30 p-2.5 backdrop-blur-sm">
      {SERVERS.map((s) => (
        <ServerTile key={s.id} server={s} />
      ))}
    </div>
  );
}

function ServerTile({ server: s }: { server: (typeof SERVERS)[number] }) {
  const [iconFailed, setIconFailed] = useState(false);
  return (
    <button
      type="button"
      title={`Join ${s.title}`}
      className={cn(
        "group relative size-11 shrink-0 overflow-hidden rounded-[var(--radius-sm)] border border-border transition",
        "hover:border-accent/60 hover:scale-[1.04]",
      )}
    >
      <span
        className={cn(
          "absolute inset-0 bg-gradient-to-br transition",
          s.color,
          iconFailed ? "opacity-80 group-hover:opacity-100" : "opacity-25 group-hover:opacity-40",
        )}
      />
      <span className="relative grid h-full place-items-center">
        {iconFailed ? (
          <span className="font-display text-sm font-bold text-white/90">{s.label}</span>
        ) : (
          <img
            src={faviconUrl(s.host)}
            alt={s.title}
            className="size-7 rounded-sm object-contain drop-shadow-[0_1px_2px_rgba(0,0,0,0.6)]"
            referrerPolicy="no-referrer"
            loading="lazy"
            draggable={false}
            onError={() => setIconFailed(true)}
          />
        )}
      </span>
    </button>
  );
}

function ResultLine({
  success,
  error,
  blocked,
}: {
  success: string | null;
  error: string | null;
  blocked: string | null;
}) {
  if (success) {
    return (
      <div className="flex items-center gap-2 font-mono text-[10px] uppercase tracking-[0.2em] text-accent">
        <Sparkles className="size-3.5" /> {success}
      </div>
    );
  }
  if (error) {
    return (
      <div className="flex items-center gap-2 font-mono text-[10px] uppercase tracking-[0.2em] text-destructive">
        <AlertTriangle className="size-3.5" /> {error}
      </div>
    );
  }
  if (blocked) {
    return (
      <div className="font-mono text-[10px] uppercase tracking-[0.2em] text-destructive">
        {blocked}
      </div>
    );
  }
  return null;
}
