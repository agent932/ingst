import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useStore, IngestPlan } from '../store/useStore';

function formatSize(bytes: number): string {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
}

function getFileName(path: string): string {
  return path.split('/').pop() || path;
}

export default function ReviewPage() {
  const { sources, destination, operation, skipDuplicates, setIngestPlan, nextStep, prevStep } = useStore();
  const [isBuilding, setIsBuilding] = useState(false);
  const [plan, setPlan] = useState<IngestPlan | null>(null);
  const [filter, setFilter] = useState<'all' | 'unknown' | 'duplicates'>('all');
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
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
