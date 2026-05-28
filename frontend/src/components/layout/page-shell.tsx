import { Header } from './header';

export function PageShell({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="flex min-w-0 flex-1 flex-col overflow-hidden">
      <Header title={title} />
      <main className="min-w-0 flex-1 overflow-auto p-3 pb-20 md:p-6">{children}</main>
    </div>
  );
}
