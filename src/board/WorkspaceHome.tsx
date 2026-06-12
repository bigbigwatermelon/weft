import { WorkspaceKanban } from "./WorkspaceKanban";

export function WorkspaceHome() {
  return (
    <section className="flex min-w-0 flex-1 flex-col overflow-hidden bg-bg">
      <WorkspaceKanban />
    </section>
  );
}
