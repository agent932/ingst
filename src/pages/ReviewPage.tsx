import { useState, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useStore, IngestPlan } from '../store/useStore';
import { formatSize } from '../utils/formatters';


function getFileName(path: string): string {
  return path.split('/').pop() || path;
}

function getFileType(path: string): string {
  const ext = (path.split('.').pop() || '').toLowerCase();
  if (['mp4', 'mov', 'mxf', 'avi', 'mkv'].includes(ext)) return 'video';
  if (['wav', 'mp3', 'aac'].includes(ext)) return 'audio';
  return 'photo';
}

const THUMB_BATCH = 8;

function FileTypeIcon({ fileType }: { fileType: string }) {
  if (fileType === 'video') {
    return (
      <svg className="w-6 h-6 text-blue-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5}
          d="M15 10l4.553-2.276A1 1 0 0121 8.723v6.554a1 1 0 01-1.447.894L15 14M3 8a2 2 0 012-2h8a2 2 0 012 2v8a2 2 0 01-2 2H5a2 2 0 01-2-2V8z" />
      </svg>
    );
  }
  if (fileType === 'audio') {
    return (
      <svg className="w-6 h-6 text-purple-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5}
          d="M9 19V6l12-3v13M9 19c0 1.105-1.343 2-3 2s-3-.895-3-2 1.343-2 3-2 3 .895 3 2zm12-3c0 1.105-1.343 2-3 2s-3-.895-3-2 1.343-2 3-2 3 .895 3 2zM9 10l12-3" />
      </svg>
    );
  }
  // photo / RAW
  return (
    <svg className="w-6 h-6 text-green-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5}
        d="M4 16l4.586-4.586a2 2 0 012.828 0L16 16m-2-2l1.586-1.586a2 2 0 012.828 0L20 14m-6-6h.01M6 20h12a2 2 0 002-2V6a2 2 0 00-2-2H6a2 2 0 00-2 2v12a2 2 0 002 2z" />
    </svg>
  );
}

function Thumbnail({ src, fileType }: { src: string | null | undefined; fileType: string }) {
  if (src === undefined) {
    // still loading
    return <div className="w-10 h-10 rounded bg-gray-200 dark:bg-slate-700 animate-pulse" />;
  }
  if (src) {
    return (
      <img
        src={src}
        alt=""
        className="w-10 h-10 rounded object-cover"
      />
    );
  }
  // null → unsupported, show icon
  return (
    <div className="w-10 h-10 rounded bg-gray-100 dark:bg-slate-800 flex items-center justify-center">
      <FileTypeIcon fileType={fileType} />
    </div>
  );
}

export default function ReviewPage() {
  const { sources, destination, operation, skipDuplicates, setIngestPlan, nextStep, prevStep } = useStore();
  const [isBuilding, setIsBuilding] = useState(false);
  const [plan, setPlan] = useState<IngestPlan | null>(null);
  const [filter, setFilter] = useState<'all' | 'unknown' | 'duplicates'>('all');
  const [error, setError] = useState<string | null>(null);
  // undefined = loading, null = unsupported/failed, string = data URL
  const [thumbnails, setThumbnails] = useState<Map<string, string | null>>(new Map());
  const thumbLoadRef = useRef(false);
  const planBuiltRef = useRef(false);

  useEffect(() => {
    // StrictMode fires mount effects twice in dev, which rescans every source
    // and rebuilds the plan a second time. The ref survives the simulated
    // remount; genuine navigation back to this step gets a fresh one and
    // rebuilds as expected. "Try Again" calls buildPlan directly.
    if (planBuiltRef.current) return;
    planBuiltRef.current = true;
    buildPlan();
  }, []);

  const buildPlan = async () => {
    setIsBuilding(true);
    setError(null);
    
    try {
      const result = await invoke<IngestPlan>('build_ingest_plan', {
        sources: sources.map(s => ({
          path: s.path,
          label: s.label,
          exclusions: s.exclusions,
        })),
        options: {
          operation,
          skip_duplicates: skipDuplicates,
          dest_root: destination,
        },
      });
      
      setPlan(result);
      setIngestPlan(result);
    } catch (e) {
      setError(`Failed to build plan: ${e}`);
    } finally {
      setIsBuilding(false);
    }
  };

  useEffect(() => {
    if (!plan || thumbLoadRef.current) return;
    thumbLoadRef.current = true;

    const paths = plan.operations
      .slice(0, 100)
      .map(op => op.source_path);

    (async () => {
      for (let i = 0; i < paths.length; i += THUMB_BATCH) {
        const batch = paths.slice(i, i + THUMB_BATCH);
        await Promise.all(
          batch.map(async (path) => {
            try {
              const thumb = await invoke<string | null>('get_thumbnail', { path });
              setThumbnails(prev => new Map(prev).set(path, thumb));
            } catch {
              setThumbnails(prev => new Map(prev).set(path, null));
            }
          })
        );
      }
    })();
  }, [plan]);

  const filteredOperations = plan?.operations.filter(op => {
    if (filter === 'unknown') {
      return op.device_name === 'UnknownDevice' || !op.capture_date;
    }
    if (filter === 'duplicates') {
      return op.action === 'skip';
    }
    return true;
  }) || [];

  if (isBuilding) {
    return (
      <div className="max-w-3xl mx-auto">
        <div className="mb-6">
          <h1 className="text-2xl font-bold text-gray-900 dark:text-white">Review Plan</h1>
          <p className="text-gray-600 dark:text-gray-400 mt-1">
            Building ingest plan...
          </p>
        </div>
        
        <div className="card p-8 text-center">
          <div className="w-8 h-8 mx-auto mb-4 border-2 border-accent border-t-transparent rounded-full animate-spin" />
          <p className="text-gray-600 dark:text-gray-400">Analyzing files and metadata...</p>
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="max-w-3xl mx-auto">
        <div className="mb-6">
          <h1 className="text-2xl font-bold text-gray-900 dark:text-white">Review Plan</h1>
        </div>
        
        <div className="card p-4 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 text-red-700 dark:text-red-400">
          {error}
        </div>
        
        <div className="mt-4 flex gap-3">
          <button onClick={buildPlan} className="btn btn-secondary">
            Try Again
          </button>
          <button onClick={prevStep} className="btn btn-secondary">
            Back
          </button>
        </div>
      </div>
    );
  }

  if (!plan) {
    return null;
  }

  return (
    <div className="max-w-5xl mx-auto">
      <div className="mb-6">
        <h1 className="text-2xl font-bold text-gray-900 dark:text-white">Review Plan</h1>
        <p className="text-gray-600 dark:text-gray-400 mt-1">
          Review the ingest plan before starting
        </p>
      </div>

      <div className="grid grid-cols-3 gap-4 mb-6">
        <div className="card p-4">
          <p className="text-sm text-gray-500 dark:text-gray-400">Files to ingest</p>
          <p className="text-2xl font-bold text-gray-900 dark:text-white">{plan.total_files}</p>
        </div>
        <div className="card p-4">
          <p className="text-sm text-gray-500 dark:text-gray-400">Total size</p>
          <p className="text-2xl font-bold text-gray-900 dark:text-white">{formatSize(plan.total_size)}</p>
        </div>
        <div className="card p-4">
          <p className="text-sm text-gray-500 dark:text-gray-400">Skipped duplicates</p>
          <p className="text-2xl font-bold text-yellow-600 dark:text-yellow-400">{plan.duplicate_count}</p>
        </div>
      </div>

      <div className="card mb-6">
        <div className="p-4 border-b border-gray-200 dark:border-slate-700">
          <div className="flex items-center justify-between">
            <h3 className="font-medium text-gray-900 dark:text-white">
              Files ({filteredOperations.length})
            </h3>
            <div className="flex gap-2">
              {(['all', 'unknown', 'duplicates'] as const).map((f) => (
                <button
                  key={f}
                  onClick={() => setFilter(f)}
                  className={`px-3 py-1 rounded text-xs font-medium capitalize ${
                    filter === f
                      ? 'bg-accent text-white'
                      : 'bg-gray-100 dark:bg-slate-700 text-gray-600 dark:text-gray-300'
                  }`}
                >
                  {f === 'all' ? 'All' : f === 'unknown' ? 'Unknown Device' : 'Duplicates'}
                </button>
              ))}
            </div>
          </div>
        </div>

        <div className="overflow-x-auto max-h-96">
          <table className="w-full text-sm">
            <thead className="bg-gray-50 dark:bg-slate-800 sticky top-0">
              <tr>
                <th className="px-3 py-3 w-14" />
                <th className="px-4 py-3 text-left text-gray-500 dark:text-gray-400 font-medium">Source</th>
                <th className="px-4 py-3 text-left text-gray-500 dark:text-gray-400 font-medium">Capture Date</th>
                <th className="px-4 py-3 text-left text-gray-500 dark:text-gray-400 font-medium">Device</th>
                <th className="px-4 py-3 text-left text-gray-500 dark:text-gray-400 font-medium">Destination</th>
                <th className="px-4 py-3 text-left text-gray-500 dark:text-gray-400 font-medium">Action</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-gray-200 dark:divide-slate-700">
              {filteredOperations.slice(0, 100).map((op, idx) => (
                <tr key={idx} className="hover:bg-gray-50 dark:hover:bg-slate-800/50">
                  <td className="px-3 py-2">
                    <Thumbnail src={thumbnails.get(op.source_path)} fileType={getFileType(op.source_path)} />
                  </td>
                  <td className="px-4 py-3 font-mono text-xs truncate max-w-[200px]" title={op.source_path}>
                    {getFileName(op.source_path)}
                  </td>
                  <td className="px-4 py-3 text-gray-600 dark:text-gray-400">
                    {op.capture_date ? op.capture_date.split('T')[0] : '-'}
                  </td>
                  <td className="px-4 py-3">
                    <span className={`px-2 py-0.5 rounded text-xs ${
                      op.device_name === 'UnknownDevice'
                        ? 'bg-yellow-100 dark:bg-yellow-900/30 text-yellow-700 dark:text-yellow-400'
                        : 'bg-gray-100 dark:bg-slate-700 text-gray-600 dark:text-gray-300'
                    }`}>
                      {op.device_name}
                    </span>
                  </td>
                  <td className="px-4 py-3 font-mono text-xs text-gray-500 dark:text-gray-400 truncate max-w-[200px]" title={op.dest_path}>
                    {getFileName(op.dest_path)}
                  </td>
                  <td className="px-4 py-3">
                    {op.action === 'skip' ? (
                      <span className="text-yellow-600 dark:text-yellow-400">Skip</span>
                    ) : (
                      <span className="text-green-600 dark:text-green-400">{operation}</span>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          {filteredOperations.length > 100 && (
            <p className="p-4 text-center text-gray-500 dark:text-gray-400 text-sm">
              Showing first 100 of {filteredOperations.length} files
            </p>
          )}
        </div>
      </div>

      <div className="flex justify-between">
        <button onClick={prevStep} className="btn btn-secondary">
          Back
        </button>
        <button onClick={nextStep} className="btn btn-primary">
          Start Ingest
        </button>
      </div>
    </div>
  );
}
