import { useStore } from '../store/useStore';

export default function OrganizePage() {
  const { operation, setOperation, skipDuplicates, setSkipDuplicates, nextStep, prevStep } = useStore();

  return (
    <div className="max-w-3xl mx-auto">
      <div className="mb-6">
        <h1 className="text-2xl font-bold text-gray-900 dark:text-white">Ingest Options</h1>
        <p className="text-gray-600 dark:text-gray-400 mt-1">
          Choose how to handle your media files
        </p>
      </div>

      <div className="card p-6 mb-6">
        <h3 className="font-medium text-gray-900 dark:text-white mb-4">Operation</h3>
        
        <div className="grid grid-cols-2 gap-4">
          <button
            onClick={() => setOperation('copy')}
            className={`p-4 rounded-lg border-2 text-left transition-all ${
              operation === 'copy'
                ? 'border-accent bg-accent/5'
                : 'border-gray-200 dark:border-slate-700 hover:border-gray-300 dark:hover:border-slate-600'
            }`}
          >
            <div className="flex items-center gap-3 mb-2">
              <div className={`w-5 h-5 rounded-full border-2 flex items-center justify-center ${
                operation === 'copy' ? 'border-accent' : 'border-gray-300 dark:border-slate-600'
              }`}>
                {operation === 'copy' && <div className="w-2.5 h-2.5 rounded-full bg-accent" />}
              </div>
              <span className="font-medium text-gray-900 dark:text-white">Copy</span>
            </div>
            <p className="text-sm text-gray-500 dark:text-gray-400">
              Keep originals in place, create copies in library
            </p>
          </button>

          <button
            onClick={() => setOperation('move')}
            className={`p-4 rounded-lg border-2 text-left transition-all ${
              operation === 'move'
                ? 'border-accent bg-accent/5'
                : 'border-gray-200 dark:border-slate-700 hover:border-gray-300 dark:hover:border-slate-600'
            }`}
          >
            <div className="flex items-center gap-3 mb-2">
              <div className={`w-5 h-5 rounded-full border-2 flex items-center justify-center ${
                operation === 'move' ? 'border-accent' : 'border-gray-300 dark:border-slate-600'
              }`}>
                {operation === 'move' && <div className="w-2.5 h-2.5 rounded-full bg-accent" />}
              </div>
              <span className="font-medium text-gray-900 dark:text-white">Move</span>
            </div>
            <p className="text-sm text-gray-500 dark:text-gray-400">
              Transfer files to library, remove from source
            </p>
          </button>
        </div>
      </div>

      <div className="card p-6 mb-6">
        <h3 className="font-medium text-gray-900 dark:text-white mb-4">Duplicate Handling</h3>
        
        <div className="flex items-center justify-between p-4 bg-gray-50 dark:bg-slate-800 rounded-lg">
          <div>
            <p className="font-medium text-gray-900 dark:text-white">Skip duplicates</p>
            <p className="text-sm text-gray-500 dark:text-gray-400">
              Skip files that already exist in the library (based on size and timestamp)
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

        {skipDuplicates && (
          <div className="mt-4 p-3 bg-green-50 dark:bg-green-900/20 border border-green-200 dark:border-green-800 rounded-lg">
            <p className="text-sm text-green-700 dark:text-green-400">
              Files will be hashed during scan and compared against previous imports to avoid duplicates.
              Great for creator workflows (YouTube, TikTok, client work).
            </p>
          </div>
        )}
      </div>

      <div className="card p-6 mb-6">
        <h3 className="font-medium text-gray-900 dark:text-white mb-4">Collision Handling</h3>
        
        <div className="p-4 bg-gray-50 dark:bg-slate-800 rounded-lg">
          <p className="text-sm text-gray-600 dark:text-gray-400">
            If a file with the same name already exists in the destination, a number will be appended:
          </p>
          <div className="mt-3 font-mono text-sm">
            <p className="text-gray-500">video.mp4</p>
            <p className="text-gray-500">video_1.mp4</p>
            <p className="text-gray-500">video_2.mp4</p>
            <p className="text-gray-500">...</p>
          </div>
        </div>
      </div>

      <div className="flex justify-between">
        <button onClick={prevStep} className="btn btn-secondary">
          Back
        </button>
        <button onClick={nextStep} className="btn btn-primary">
          Review Plan
        </button>
      </div>
    </div>
  );
}
