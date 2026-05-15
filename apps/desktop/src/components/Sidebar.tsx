import { Home, Layers, Settings } from "lucide-react";
import { cn } from "@/lib/cn";
import { type Page, useNav } from "@/stores/nav";

const top: { id: Page; label: string; icon: typeof Home }[] = [
  { id: "home", label: "Home", icon: Home },
  { id: "versions", label: "Versions", icon: Layers },
];

export function Sidebar() {
  const { page, go } = useNav();
  return (
    <nav className="flex h-screen w-16 shrink-0 flex-col items-center border-r border-border bg-base/60 backdrop-blur-md">
      <a
        className="group flex h-16 w-full items-center justify-center"
        href="https://flarial.xyz"
        target="_blank"
        rel="noreferrer"
        title="Flarial"
      >
        <img
          src="/logo.svg"
          alt="Flarial"
          className="size-9 transition group-hover:scale-110"
          draggable={false}
        />
      </a>

      <div className="mt-2 flex flex-col gap-1">
        {top.map((it) => (
          <NavButton key={it.id} {...it} active={page === it.id} onClick={() => go(it.id)} />
        ))}
      </div>

      <div className="mt-auto mb-3 flex flex-col gap-1">
        <NavButton
          id="settings"
          label="Settings"
          icon={Settings}
          active={page === "settings"}
          onClick={() => go("settings")}
        />
      </div>
    </nav>
  );
}

function NavButton({
  label,
  icon: Icon,
  active,
  onClick,
}: {
  id: Page;
  label: string;
  icon: typeof Home;
  active: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      title={label}
      className={cn(
        "group relative grid size-11 place-items-center rounded-[var(--radius-sm)] transition",
        active
          ? "bg-elevated text-foreground"
          : "text-faint hover:bg-elevated/60 hover:text-foreground",
      )}
    >
      <Icon className={cn("size-[18px]", active && "text-accent")} />
      {active && (
        <span className="absolute -left-3 h-5 w-[2px] rounded-r-full bg-accent shadow-[0_0_8px] shadow-accent" />
      )}
    </button>
  );
}
