import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Loader2, CheckCircle2, Download, Circle } from "lucide-react";
import { commands, type SupportedVersion } from "@/ipc/bindings";
import { cn } from "@/lib/cn";

export function VersionsPage() {
  const qc = useQueryClient();
  const pkg = useQuery({
    queryKey: ["minecraft", "package"],
    queryFn: commands.getMinecraftPackage,
  });
  const versions = useQuery({
    queryKey: ["versions"],
    queryFn: commands.listVersions,
  });

  const switchTo = useMutation({
    mutationFn: (v: string) => commands.installVersion(v),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["minecraft"] });
      qc.invalidateQueries({ queryKey: ["versions"] });
    },
  });

  const installed = pkg.data?.version ?? null;

  return (
    <div className="flex h-screen flex-col">
      <header className="sticky top-0 z-10 flex items-end justify-between border-b border-border bg-base/70 px-10 pt-8 pb-4 backdrop-blur-md">
        <div>
          <div className="font-mono text-[10px] uppercase tracking-[0.22em] text-faint">
            Versions
          </div>
          <h2 className="font-display mt-1 text-2xl font-semibold tracking-tight">
            Switch Minecraft build
          </h2>
        </div>
        <div className="font-mono text-[10px] uppercase tracking-[0.22em] text-faint">
          installed · <span className="text-foreground">{installed ?? "—"}</span>
        </div>
      </header>

      <div className="min-h-0 flex-1 overflow-y-auto px-10 py-6">
        {versions.isLoading && (
          <div className="flex items-center gap-2 text-muted">
            <Loader2 className="size-4 animate-spin" /> Loading versions…
          </div>
        )}

        {versions.error && (
          <div className="text-destructive">{String(versions.error)}</div>
        )}

        {versions.data && (
          <ul className="space-y-1.5">
            {versions.data.map((v) => (
              <VersionRow
                key={v.version}
                v={v}
                installed={installed === v.version}
                busy={switchTo.isPending && switchTo.variables === v.version}
                disabled={switchTo.isPending}
                onSwitch={() => switchTo.mutate(v.version)}
              />
            ))}
          </ul>
        )}

        {switchTo.isError && (
          <div className="sticky bottom-4 mx-auto mt-6 max-w-md rounded-[var(--radius-sm)] border border-destructive/40 bg-base/90 px-4 py-3 text-center text-sm text-destructive backdrop-blur">
            {String(switchTo.error)}
          </div>
        )}
      </div>
    </div>
  );
}

function VersionRow({
  v,
  installed,
  busy,
  disabled,
  onSwitch,
}: {
  v: SupportedVersion;
  installed: boolean;
  busy: boolean;
  disabled: boolean;
  onSwitch: () => void;
}) {
  return (
    <li
      className={cn(
        "flex items-center gap-4 rounded-[var(--radius-sm)] border bg-elevated/40 px-4 py-3 transition",
        installed
          ? "border-accent/40 bg-accent/10"
          : "border-border hover:bg-elevated/70",
      )}
    >
      <span className="grid size-5 place-items-center">
        {installed ? (
          <CheckCircle2 className="size-5 text-accent" />
        ) : (
          <Circle className="size-4 text-faint" />
        )}
      </span>

      <span className="font-mono text-sm tabular-nums">{v.version}</span>

      <span
        className={cn(
          "font-mono text-[10px] uppercase tracking-[0.18em]",
          v.supported ? "text-muted" : "text-faint",
        )}
      >
        {v.supported ? "supported" : "unsupported"}
      </span>

      <div className="flex-1" />

      {installed ? (
        <span className="font-mono text-[10px] uppercase tracking-[0.22em] text-accent">
          current
        </span>
      ) : (
        <button
          type="button"
          onClick={onSwitch}
          disabled={disabled || !v.supported}
          className={cn(
            "inline-flex items-center gap-1.5 rounded-[var(--radius-sm)] border border-border px-3 py-1.5",
            "font-mono text-[10px] uppercase tracking-[0.22em] text-foreground transition",
            "hover:border-accent/60 hover:text-accent",
            "disabled:cursor-not-allowed disabled:opacity-40 disabled:hover:border-border disabled:hover:text-foreground",
          )}
        >
          {busy ? (
            <Loader2 className="size-3 animate-spin" />
          ) : (
            <Download className="size-3" />
          )}
          {busy ? "Installing" : "Install"}
        </button>
      )}
    </li>
  );
}
