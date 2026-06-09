import { useStore } from "../state/store";
import { RepoMapView } from "./RepoMapView";
import { WorkspaceKanban } from "./WorkspaceKanban";

/**
 * The workspace home (no issue open). It stays portfolio-level: issue board or
 * repo map. The console belongs inside an issue, not at workspace scope.
 */
export function WorkspaceHome() {
  const { homeTab } = useStore();

  // No local page header — the shell topbar owns window chrome.
  return (
    <section className="flex min-w-0 flex-1 flex-col overflow-hidden bg-bg">
      {homeTab === "repos" ? <RepoMapView embedded /> : <WorkspaceKanban />}
    </section>
  );
}
