import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { useStore, IngestResult, ProgressEvent } from '../store/useStore';

function formatSize(bytes: number): string {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
}

function formatTime(secs: number): string {
  const hours = Math.floor(secs / 3600);
  const mins = Math.floor((secs % 3600) / 60);
  const s = secs % 60;
  
  if (hours > 0) {
    return `${hours}:${mins.toString().padStart(2, '0')}:${s.toString().padStart(2, '0')}`;
  }
  return `${mins}:${s.toString().padStart(2, '0')}`;
}

function getFileName(path: string): string {
  return path.split('/').pop() || path;
}

export default function IngestPage() {
  const { 
    ingestPlan, 
    destination, 
    operation, 
    skipDuplicates, 
    progress, 
    setProgress,
    setIngestResult,
    setIsIngesting,
    isIngesting,
    nextStep,
  } = useStore();
  
  const [error, setError] = useState<string | null>(null);
  const [isComplete, setIsComplete] = useState(false);

  useEffect(() => {
    startIngest();
    
    const unlisten = listen<ProgressEvent>('ingest-progress', (event) => {
      setProgress(event.payload);
      if (event.payload.status === 'complete') {
        setIsComplete(true);
      }
    });
    
    return () => {
      unlisten.then(fn => fn());
    };
  }, []);

  const startIngest = async () => {
    if (!ingestPlan) return;
    
    setIsIngesting(true);
    setError(null);
    
    try {
      const result = await invoke<IngestResult>('execute_ingest', {
        plan: ingestPlan,
        options: {
          operation,
          skip_duplicates: skipDuplicates,
          dest_root: destination,
        },
      });
      
      setIngestResult(result);
    } catch (e) {
      setError(`Ingest failed: ${e}`);
    } finally {
      setIsIngesting(false);
    }
  };

  const handleCancel = async () => {
    try {
      await invoke('cancel_ingest');
    } catch (e) {
      console.error('Failed to cancel:', e);
    }
  };

  const progressPercent = progress 
    ? Math.round((progress.current_index / progress.total) * 100)
    : 0;
  
  const speed = progress && progress.elapsed_secs > 0
    ? Math.round(progress.bytes_copied / progress.elapsed_secs / 1024 / 1024)
    : 0;

  if (error) {
    return (
      <div className="max-w-3xl mx-auto">
        <div className="mb-6">
          <h1 className="text-2xl font-bold text-gray-900 dark:text-white">Ingest Error</h1>
        </div>
        
        <div className="card p-4 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 text-red-700 dark:text-red-400">
          {error}
        </div>
      </div>
    );
  }

  return (
    <div className="max-w-3xl mx-auto">
      <div className="mb-6">
        <h1 className="text-2xl font-bold text-gray-900 dark:text-white">
          {isComplete ? 'Ingest Complete' : 'Ingesting...'}
        </h1>
        <p className="text-gray-600 dark:text-gray-400 mt-1">
          {operation === 'copy' ? 'Copying' : 'Moving'} files to your library
        </p>
      </div>

      <div className="card p-6 mb-6">
        <div className="mb-4">
          <div className="flex justify-between text-sm mb-2">
            <span className="text-gray-600 dark:text-gray-400">
              {progress?.current_index || 0} of {progress?.total || 0} files
            </span>
            <span className="font-medium text-gray-900 dark:text-white">{progressPercent}%</span>
          </div>
          <div className="h-3 bg-gray-200 dark:bg-slate-700 rounded-full overflow-hidden">
            <div
              className="h-full bg-accent transition-all duration-300"
              style={{ width: `${progressPercent}%` }}
            />
          </div>
        </div>

        <div className="mb-4">
          <p className="text-sm text-gray-500 dark:text-gray-400 mb-1">Current file</p>
          <p className="font-mono text-sm text-gray-900 dark:text-white truncate" title={progress?.current_file}>
            {progress?.current_file ? getFileName(progress.current_file) : '-'}
          </p>
        </div>

        <div className="grid grid-cols-3 gap-4 text-sm">
          <div>
            <p className="text-gray-500 dark:text-gray-400">Elapsed</p>
            <p className="font-medium text-gray-900 dark:text-white">
              {formatTime(progress?.elapsed_secs || 0)}
            </p>
          </div>
          <div>
            <p className="text-gray-500 dark:text-gray-400">Speed</p>
            <p className="font-medium text-gray-900 dark:text-white">
              {speed > 0 ? `${speed} MB/s` : '-'}
            </p>
          </div>
          <div>
            <p className="text-gray-500 dark:text-gray-400">Processed</p>
            <p className="font-medium text-gray-900 dark:text-white">
              {progress ? formatSize(progress.bytes_copied) : '-'}
            </p>
          </div>
        </div>
      </div>

      {!isComplete && isIngesting && (
        <div className="flex justify-center">
          <button
            onClick={handleCancel}
            className="btn btn-secondary text-red-600 dark:text-red-400"
          >
            Cancel
          </button>
        </div>
      )}

      {isComplete && (
        <div className="flex justify-end">
          <button onClick={nextStep} className="btn btn-primary">
            View Summary
          </button>
        </div>
      )}
    </div>
  );
}
