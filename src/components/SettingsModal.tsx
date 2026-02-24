import { useStore } from '../store/useStore';

export default function SettingsModal({ onClose }: { onClose: () => void }) {
  const { theme, setTheme, operation, setOperation, skipDuplicates, setSkipDuplicates } = useStore();

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
      <div className="card w-full max-w-md p-6">
        <div className="flex items-center justify-between mb-6">
          <h2 className="text-xl font-semibold text-gray-900 dark:text-white">Settings</h2>
          <button
            onClick={onClose}
            className="p-1 rounded hover:bg-gray-100 dark:hover:bg-slate-700"
          >
            <svg className="w-5 h-5 text-gray-500" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>

        <div className="space-y-6">
          <div>
            <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
              Theme
            </label>
            <div className="flex gap-2">
              {(['light', 'dark', 'system'] as const).map((t) => (
                <button
                  key={t}
                  onClick={() => setTheme(t)}
                  className={`flex-1 py-2 px-3 rounded-md text-sm font-medium capitalize transition-colors ${
                    theme === t
                      ? 'bg-accent text-white'
                      : 'bg-gray-100 dark:bg-slate-700 text-gray-700 dark:text-gray-300 hover:bg-gray-200 dark:hover:bg-slate-600'
                  }`}
                >
                  {t}
                </button>
              ))}
            </div>
          </div>

          <div>
            <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
              Default Operation
            </label>
            <div className="flex gap-2">
              <button
                onClick={() => setOperation('copy')}
                className={`flex-1 py-2 px-3 rounded-md text-sm font-medium transition-colors ${
                  operation === 'copy'
                    ? 'bg-accent text-white'
                    : 'bg-gray-100 dark:bg-slate-700 text-gray-700 dark:text-gray-300 hover:bg-gray-200 dark:hover:bg-slate-600'
                }`}
              >
                Copy
              </button>
              <button
                onClick={() => setOperation('move')}
                className={`flex-1 py-2 px-3 rounded-md text-sm font-medium transition-colors ${
                  operation === 'move'
                    ? 'bg-accent text-white'
                    : 'bg-gray-100 dark:bg-slate-700 text-gray-700 dark:text-gray-300 hover:bg-gray-200 dark:hover:bg-slate-600'
                }`}
              >
                Move
              </button>
            </div>
            <p className="mt-1 text-xs text-gray-500 dark:text-gray-400">
              Copy keeps originals, move transfers them
            </p>
          </div>

          <div className="flex items-center justify-between">
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300">
                Skip Duplicates
              </label>
              <p className="text-xs text-gray-500 dark:text-gray-400">
                Skip files that already exist in the library
              </p>
            </div>
            <button
              onClick={() => setSkipDuplicates(!skipDuplicates)}
              className={`relative w-11 h-6 rounded-full transition-colors ${
                skipDuplicates ? 'bg-accent' : 'bg-gray-300 dark:bg-slate-600'
              }`}
            >
              <span
                className={`absolute top-0.5 left-0.5 w-5 h-5 bg-white rounded-full shadow transition-transform ${
                  skipDuplicates ? 'translate-x-5' : 'translate-x-0'
                }`}
              />
            </button>
          </div>
        </div>

        <div className="mt-6 pt-4 border-t border-gray-200 dark:border-slate-700">
          <p className="text-xs text-center text-gray-500 dark:text-gray-400">
            Ingst v0.1.0 — Made for creators
          </p>
        </div>
      </div>
    </div>
  );
}
