import { StoreProvider, useStore } from "./state/store";
import { WorkspaceNav } from "./nav/WorkspaceNav";
import { ThreadBoard } from "./board/ThreadBoard";
import { WorkspaceBoard } from "./board/WorkspaceBoard";
import { NeedsYouView } from "./board/NeedsYouView";
import { RepoMapView } from "./board/RepoMapView";
import { SessionView } from "./session/SessionView";

function Main() {
  const { activeSessionId, activeThreadId, showNeeds, showRepoMap } = useStore();
  if (showRepoMap) return <RepoMapView />;
  if (showNeeds) return <NeedsYouView />;
  if (activeSessionId != null) return <SessionView />;
  if (activeThreadId != null) return <ThreadBoard />;
  return <WorkspaceBoard />;
}

export default function App() {
  return (
    <StoreProvider>
      <div className="flex h-screen w-screen overflow-hidden bg-bg text-ink">
        <WorkspaceNav />
        <Main />
      </div>
    </StoreProvider>
  );
}
