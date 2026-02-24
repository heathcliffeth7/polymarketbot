import { Header } from './header';

export function PageShell({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="flex flex-1 flex-col overflow-hidden">
      <Header title={title} />
      <main className="flex-1 overflow-auto p-6">{children}</main>
    </div>
  );
}
