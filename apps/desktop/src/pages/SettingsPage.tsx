import { useEffect, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Check, Loader2 } from "lucide-react";
import { commands, type AppSettings } from "@/ipc/bindings";
import { cn } from "@/lib/cn";

export function SettingsPage() {
  const qc = useQueryClient();
  const settings = useQuery({
    queryKey: ["settings"],
    queryFn: commands.loadSettings,
  });
  const [draft, setDraft] = useState<AppSettings | null>(null);
  const [savedAt, setSavedAt] = useState<number | null>(null);
  const debounce = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    if (settings.data && !draft) setDraft(settings.data);
  }, [settings.data, draft]);

  const save = useMutation({
    mutationFn: commands.saveSettings,
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["settings"] });
      setSavedAt(Date.now());
    },
  });

  function patch(next: AppSettings) {
    setDraft(next);
    if (debounce.current) clearTimeout(debounce.current);
    debounce.current = setTimeout(() => save.mutate(next), 300);
  }

  if (!draft) return <div className="p-8 text-muted">Loading…</div>;

  return (
    <div className="flex h-screen flex-col">
      <header className="flex items-center justify-between border-b border-border bg-base/70 px-10 pt-7 pb-4 backdrop-blur-md">
        <div>
          <div className="font-mono text-[10px] uppercase tracking-[0.22em] text-faint">
            Settings
          </div>
          <h2 className="font-display mt-1 text-2xl font-semibold tracking-tight">
            Preferences
          </h2>
        </div>
        <AutosaveBadge pending={save.isPending} savedAt={savedAt} error={save.isError} />
      </header>

      <div className="min-h-0 flex-1 overflow-y-auto px-10 py-6">
        <div className="overflow-hidden rounded-[var(--radius)] border border-border bg-elevated/40 backdrop-blur-sm">
          <Toggle
            label="Use custom DLL"
            description="Inject a DLL of your choice instead of the official Flarial client."
            checked={draft.UseCustomDll}
            onChange={(v) => patch({ ...draft, UseCustomDll: v })}
          />

          {draft.UseCustomDll && (
            <div className="border-t border-border px-6 py-5">
              <label className="block">
                <span className="font-mono text-[10px] uppercase tracking-[0.22em] text-faint">
                  Custom DLL path
                </span>
                <input
                  type="text"
                  value={draft.CustomDllPath}
                  onChange={(e) =>
                    patch({ ...draft, CustomDllPath: e.target.value })
                  }
                  placeholder="C:\\path\\to\\custom.dll"
                  className={cn(
                    "mt-2 w-full rounded-[var(--radius-sm)] border border-border bg-base px-3 py-2",
                    "font-mono text-sm text-foreground placeholder:text-faint",
                    "outline-none transition focus:border-accent",
                  )}
                />
              </label>
            </div>
          )}
        </div>

        <p className="mt-3 font-mono text-[10px] uppercase tracking-[0.2em] text-faint">
          launcher updates · automatic
        </p>
      </div>
    </div>
  );
}

function AutosaveBadge({
  pending,
  savedAt,
  error,
}: {
  pending: boolean;
  savedAt: number | null;
  error: boolean;
}) {
  const [showSaved, setShowSaved] = useState(false);
  useEffect(() => {
    if (!savedAt) return;
    setShowSaved(true);
    const t = setTimeout(() => setShowSaved(false), 1800);
    return () => clearTimeout(t);
  }, [savedAt]);

  if (error) {
    return (
      <span className="font-mono text-[10px] uppercase tracking-[0.22em] text-destructive">
        save failed
      </span>
    );
  }
  if (pending) {
    return (
      <span className="flex items-center gap-1.5 font-mono text-[10px] uppercase tracking-[0.22em] text-faint">
        <Loader2 className="size-3 animate-spin" /> saving
      </span>
    );
  }
  if (showSaved) {
    return (
      <span className="flex items-center gap-1.5 font-mono text-[10px] uppercase tracking-[0.22em] text-accent">
        <Check className="size-3" /> saved
      </span>
    );
  }
  return (
    <span className="font-mono text-[10px] uppercase tracking-[0.22em] text-faint">
      autosaved
    </span>
  );
}

type ToggleProps = {
  label: string;
  description: string;
  checked: boolean;
  onChange: (v: boolean) => void;
};

function Toggle({ label, description, checked, onChange }: ToggleProps) {
  return (
    <div className="flex items-center justify-between gap-6 px-6 py-5 border-b border-border last:border-b-0">
      <div>
        <div className="font-display text-sm font-semibold tracking-tight text-foreground">
          {label}
        </div>
        <div className="mt-0.5 text-sm text-muted">{description}</div>
      </div>
      <button
        type="button"
        role="switch"
        aria-checked={checked}
        onClick={() => onChange(!checked)}
        className={cn(
          "relative h-6 w-11 shrink-0 rounded-full transition",
          checked
            ? "bg-accent shadow-[0_0_18px] shadow-accent/40"
            : "bg-border-strong",
        )}
      >
        <span
          className={cn(
            "absolute top-0.5 size-5 rounded-full bg-foreground transition",
            checked ? "left-[1.375rem]" : "left-0.5",
          )}
        />
      </button>
    </div>
  );
}
