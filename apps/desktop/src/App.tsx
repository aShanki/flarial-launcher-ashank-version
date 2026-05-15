import { useEffect } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { invoke } from "@tauri-apps/api/core";
import { AdRail } from "@/components/AdRail";
import { Sidebar } from "@/components/Sidebar";
import { HomePage } from "@/pages/HomePage";
import { VersionsPage } from "@/pages/VersionsPage";
import { SettingsPage } from "@/pages/SettingsPage";
import { useNav } from "@/stores/nav";

export function App() {
  const page = useNav((s) => s.page);
  const direction = useNav((s) => s.direction);

  // Attach the native AdSense WebView2 child only AFTER React has mounted —
  // ensures the main WebView2 is fully alive so Tauri can host a second view
  // without raising HRESULT 0x8007139F from `Window::add_child`.
  useEffect(() => {
    const frame = requestAnimationFrame(() => {
      invoke("attach_ad_webview").catch((err) => {
        console.error("attach_ad_webview failed", err);
      });
    });
    return () => cancelAnimationFrame(frame);
  }, []);

  return (
    <>
      <div className="vignette" aria-hidden />
      <div className="app-shell flex min-h-screen">
        <Sidebar />
        <main className="relative flex-1 overflow-hidden">
          <AnimatePresence mode="wait" initial={false} custom={direction}>
            <motion.div
              key={page}
              custom={direction}
              variants={{
                initial: (d: number) => ({
                  opacity: 0,
                  y: d > 0 ? 24 : -24,
                  filter: "blur(6px)",
                }),
                animate: { opacity: 1, y: 0, filter: "blur(0px)" },
                exit: (d: number) => ({
                  opacity: 0,
                  y: d > 0 ? -24 : 24,
                  filter: "blur(6px)",
                }),
              }}
              initial="initial"
              animate="animate"
              exit="exit"
              transition={{ duration: 0.24, ease: [0.4, 0, 0.2, 1] }}
              className="h-full overflow-y-auto"
            >
              {page === "home" && <HomePage />}
              {page === "versions" && <VersionsPage />}
              {page === "settings" && <SettingsPage />}
            </motion.div>
          </AnimatePresence>
        </main>
        <AdRail />
      </div>
    </>
  );
}
