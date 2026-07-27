/**
 * The dimmed full-screen overlay and centered panel shared by every dialog in
 * the app. Only the chrome lives here — headers differ between dialogs (some
 * carry a close button, some do not), so each caller renders its own.
 */
export default function Modal({ children }: { children: React.ReactNode }) {
  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
      <div className="card w-full max-w-md p-6">
        {children}
      </div>
    </div>
  );
}
