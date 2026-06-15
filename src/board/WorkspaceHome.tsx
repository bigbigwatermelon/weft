import { useStore } from "../state/store";
import { RepoMapView } from "./RepoMapView";
import { WorkspaceKanban } from "./WorkspaceKanban";

export function WorkspaceHome() {
  const { homeTab } = useStore();

  return (
    <section className="flex min-w-0 flex-1 flex-col overflow-hidden bg-bg">
      {homeTab === "repos" ? <RepoMapView embedded /> : <WorkspaceKanban />}
    </section>
  );
}
