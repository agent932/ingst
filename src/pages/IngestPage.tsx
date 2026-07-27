import { useEffect, useState, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { useStore, IngestResult, ProgressEvent } from '../store/useStore';
import { formatSize } from '../utils/formatters';


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

function getFolderName(path: string): string {
  const parts = path.split('/');
  return parts.length > 1 ? parts.slice(-2, -1)[0] || '' : '';
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
    setIngestStarted,
    isIngesting,
    ingestResult,
    nextStep,
  } = useStore();

  const [error, setError] = useState<string | null>(null);
  const [sawCompleteEvent, setSawCompleteEvent] = useState(false);
  const [isPaused, setIsPaused] = useState(false);
  const lastUpdateRef = useRef<number>(0);

  // Derived rather than local-only, so returning to this step after the
  // transfer finished still shows the completed state.
  const isComplete =
    sawCompleteEvent || ingestResult !== null || progress?.status === 'complete';

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let disposed = false;

    listen<ProgressEvent>('ingest-progress', (event) => {
      // Throttle updates to every 100ms for smooth UI
      const now = Date.now();
      if (now - lastUpdateRef.current < 100 && event.payload.status !== 'complete') {
        return;
      }
      lastUpdateRef.current = now;

      setProgress(event.payload);
      if (event.payload.status === 'complete') {
        setSawCompleteEvent(true);
      }
    }).then((fn) => {
      if (disposed) {
        fn();
        return;
      }
      unlisten = fn;

      // Launch the transfer at most once per plan. Guards both StrictMode's
      // double-mount in dev and navigating back to this step later, either of
      // which would otherwise re-run the entire ingest over the same files.
      if (!useStore.getState().ingestStarted) {
        setIngestStarted(true);
        startIngest();
      }
    });

    return () => {
      disposed = true;
      unlisten?.();
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

  const handlePauseResume = async () => {
    try {
      if (isPaused) {
        await invoke('resume_ingest');
        setIsPaused(false);
      } else {
        await invoke('pause_ingest');
        setIsPaused(true);
      }
    } catch (e) {
      console.error('Failed to pause/resume:', e);
    }
  };

  const progressPercent = progress 
    ? Math.round(((progress.current_index - 1) / Math.max(progress.total, 1)) * 100)
    : 0;
  
  const elapsedSecs = progress?.elapsed_secs || 0;
  const speed = elapsedSecs > 0 && progress && progress.bytes_copied > 0
    ? Math.round(progress.bytes_copied / elapsedSecs / 1024 / 1024)
    : 0;
  
  // Calculate current file progress
  const currentFileProgress = progress && progress.current_file_total > 0
    ? Math.round((progress.current_file_bytes / progress.current_file_total) * 100)
    : 0;
  
// Calculate ETA
  const remainingBytes = progress ? progress.total_bytes - progress.bytes_copied : 0;
  const etaSecs = speed > 0 && remainingBytes > 0 ? Math.round(remainingBytes / (speed * 1024 * 1024)) : null;
  
  const getStatusText = () => {
    if (!progress) return 'Preparing...';
    switch (progress.status) {
      case 'starting': return 'Preparing files...';
      case 'processing': return `${operation === 'copy' ? 'Copying' : 'Moving'} file ${progress.current_index} of ${progress.total}`;
      case 'complete': return 'Complete!';
      default: return progress.status;
    }
  };

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
          {getStatusText()}
        </p>
      </div>

      {/* Live region for screen readers */}
      <div aria-live="polite" aria-atomic="true" className="sr-only">
        {isComplete
          ? 'Ingest complete.'
          : isPaused
          ? 'Ingest paused.'
          : progress
          ? `${progress.current_index} of ${progress.total} files processed, ${progressPercent} percent complete.`
          : 'Preparing ingest...'}
      </div>

      <div className="card p-6 mb-6">
        {/* Status indicator */}
        <div className="flex items-center gap-2 mb-4">
          <div className={`w-3 h-3 rounded-full ${isComplete ? 'bg-green-500' : isPaused ? 'bg-yellow-500' : 'bg-accent animate-pulse'}`} />
          <span className="text-sm text-gray-600 dark:text-gray-400">
            {isComplete ? 'All files processed' : isPaused ? 'Paused' : 'Processing...'}
          </span>
        </div>

        {/* Overall progress bar */}
        <div className="mb-6">
          <div className="flex justify-between text-sm mb-2">
            <span className="text-gray-600 dark:text-gray-400">
              {progress?.current_index || 0} of {progress?.total || 0} files
            </span>
            <span className="font-medium text-gray-900 dark:text-white">{progressPercent}%</span>
          </div>
          <div className="h-3 bg-gray-200 dark:bg-slate-700 rounded-full overflow-hidden">
            <div
              className="h-full bg-accent transition-all duration-200"
              style={{ width: `${progressPercent}%` }}
            />
          </div>
        </div>

        {/* Current file */}
        <div className="mb-6 p-4 bg-gray-50 dark:bg-slate-800 rounded-lg">
          <div className="flex items-center justify-between mb-2">
            <div className="flex items-center gap-2">
              <svg className="w-4 h-4 text-accent" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
              </svg>
              <span className="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase">Current File</span>
            </div>
            {progress && progress.current_file_total > 0 && (
              <span className="text-xs font-medium text-accent">
                {currentFileProgress}% ({formatSize(progress.current_file_bytes)} / {formatSize(progress.current_file_total)})
              </span>
            )}
          </div>
          
          <p className="font-mono text-sm text-gray-900 dark:text-white truncate" title={progress?.current_file}>
            {progress?.current_file ? getFileName(progress.current_file) : '-'}
          </p>
          {progress?.current_file && (
            <p className="text-xs text-gray-500 dark:text-gray-400 mt-1">
              {getFolderName(progress.current_file)}
            </p>
          )}
          
          {/* Current file progress bar */}
          {progress && progress.current_file_total > 0 && (
            <div className="mt-3">
              <div className="h-1.5 bg-gray-200 dark:bg-slate-700 rounded-full overflow-hidden">
                <div
                  className="h-full bg-green-500 transition-all duration-300"
                  style={{ width: `${currentFileProgress}%` }}
                />
              </div>
            </div>
          )}
        </div>

        {/* Stats grid */}
        <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
          <div className="text-center p-3 bg-gray-50 dark:bg-slate-800 rounded-lg">
            <p className="text-2xl font-bold text-gray-900 dark:text-white">
              {progress?.current_index || 0}
            </p>
            <p className="text-xs text-gray-500 dark:text-gray-400">Files Done</p>
          </div>
          <div className="text-center p-3 bg-gray-50 dark:bg-slate-800 rounded-lg">
            <p className="text-2xl font-bold text-gray-900 dark:text-white">
              {progress ? formatSize(progress.bytes_copied) : '-'}
            </p>
            <p className="text-xs text-gray-500 dark:text-gray-400">Transferred</p>
          </div>
          <div className="text-center p-3 bg-gray-50 dark:bg-slate-800 rounded-lg">
            <p className="text-2xl font-bold text-gray-900 dark:text-white">
              {speed > 0 ? `${speed} MB/s` : '-'}
            </p>
            <p className="text-xs text-gray-500 dark:text-gray-400">Speed</p>
          </div>
          <div className="text-center p-3 bg-gray-50 dark:bg-slate-800 rounded-lg">
            <p className="text-2xl font-bold text-gray-900 dark:text-white">
              {etaSecs !== null ? formatTime(etaSecs) : '-'}
            </p>
            <p className="text-xs text-gray-500 dark:text-gray-400">ETA</p>
          </div>
        </div>

        {/* Elapsed time */}
        <div className="mt-4 pt-4 border-t border-gray-200 dark:border-slate-700">
          <div className="flex justify-between text-sm">
            <span className="text-gray-500 dark:text-gray-400">Total progress</span>
            <span className="text-gray-900 dark:text-white">
              {progress ? formatSize(progress.bytes_copied) : '0 B'} / {progress ? formatSize(progress.total_bytes) : '0 B'}
            </span>
          </div>
          <div className="mt-2 h-2 bg-gray-200 dark:bg-slate-700 rounded-full overflow-hidden">
            <div
              className="h-full bg-green-500 transition-all duration-200"
              style={{ width: `${progress && progress.total_bytes > 0 ? (progress.bytes_copied / progress.total_bytes) * 100 : 0}%` }}
            />
          </div>
        </div>
      </div>

      {!isComplete && isIngesting && (
        <div className="flex justify-center gap-3">
          <button
            onClick={handlePauseResume}
            className="btn btn-secondary"
          >
            {isPaused ? 'Resume' : 'Pause'}
          </button>
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
