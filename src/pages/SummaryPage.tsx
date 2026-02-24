import { useStore } from '../store/useStore';

export default function SummaryPage() {
  const { 
    ingestResult, 
    destination, 
    reset, 
    saveSettings,
    setCurrentStep,
  } = useStore();

  const handleOpenLogFolder = async () => {
    if (ingestResult?.log_path) {
      const logDir = ingestResult.log_path.split('/').slice(0, -1).join('/');
      try {
        const { open } = await import('@tauri-apps/plugin-shell');
        await open(logDir);
      } catch (e) {
        console.error('Failed to open log folder:', e);
      }
    }
  };

  const handleOpenDestFolder = async () => {
    if (destination) {
      try {
        const { open } = await import('@tauri-apps/plugin-shell');
        await open(destination);
      } catch (e) {
        console.error('Failed to open destination folder:', e);
      }
    }
  };

  const handleNewIngest = async () => {
    await saveSettings();
    reset();
    setCurrentStep(1);
  };

  return (
    <div className="max-w-3xl mx-auto">
      <div className="mb-6">
        <h1 className="text-2xl font-bold text-gray-900 dark:text-white">Ingest Complete</h1>
        <p className="text-gray-600 dark:text-gray-400 mt-1">
          Your media has been organized into your library
        </p>
      </div>

      <div className="grid grid-cols-3 gap-4 mb-6">
        <div className="card p-4 text-center">
          <div className="w-12 h-12 mx-auto mb-2 rounded-full bg-green-100 dark:bg-green-900/30 flex items-center justify-center">
            <svg className="w-6 h-6 text-green-600 dark:text-green-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
            </svg>
          </div>
          <p className="text-2xl font-bold text-gray-900 dark:text-white">
            {ingestResult?.success_count || 0}
          </p>
          <p className="text-sm text-gray-500 dark:text-gray-400">Ingested</p>
        </div>
        
        <div className="card p-4 text-center">
          <div className="w-12 h-12 mx-auto mb-2 rounded-full bg-yellow-100 dark:bg-yellow-900/30 flex items-center justify-center">
            <svg className="w-6 h-6 text-yellow-600 dark:text-yellow-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
            </svg>
          </div>
          <p className="text-2xl font-bold text-gray-900 dark:text-white">
            {ingestResult?.skipped_count || 0}
          </p>
          <p className="text-sm text-gray-500 dark:text-gray-400">Skipped</p>
        </div>
        
        <div className="card p-4 text-center">
          <div className="w-12 h-12 mx-auto mb-2 rounded-full bg-red-100 dark:bg-red-900/30 flex items-center justify-center">
            <svg className="w-6 h-6 text-red-600 dark:text-red-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
            </svg>
          </div>
          <p className="text-2xl font-bold text-gray-900 dark:text-white">
            {ingestResult?.error_count || 0}
          </p>
          <p className="text-sm text-gray-500 dark:text-gray-400">Errors</p>
        </div>
      </div>

      {ingestResult && ingestResult.errors.length > 0 && (
        <div className="card p-4 mb-6">
          <h3 className="font-medium text-gray-900 dark:text-white mb-3">Errors</h3>
          <div className="max-h-48 overflow-y-auto space-y-2">
            {ingestResult.errors.map((err, idx) => (
              <div key={idx} className="p-2 bg-red-50 dark:bg-red-900/20 rounded text-sm text-red-700 dark:text-red-400 font-mono">
                {err}
              </div>
            ))}
          </div>
        </div>
      )}

      <div className="card p-6 mb-6">
        <h3 className="font-medium text-gray-900 dark:text-white mb-3">Log File</h3>
        <p className="text-sm text-gray-500 dark:text-gray-400 mb-3">
          A detailed log has been saved to your library
        </p>
        <div className="flex gap-3">
          <button
            onClick={handleOpenLogFolder}
            className="btn btn-secondary"
          >
            Open Log Folder
          </button>
          <button
            onClick={handleOpenDestFolder}
            className="btn btn-secondary"
          >
            Open Library
          </button>
        </div>
      </div>

      <div className="flex justify-end gap-3">
        <button onClick={handleNewIngest} className="btn btn-primary">
          Start New Ingest
        </button>
      </div>
    </div>
  );
}
