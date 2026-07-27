import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { useStore, Source } from '../store/useStore';
import { formatSize } from '../utils/formatters';

interface SourceScanResult {
  source: Source;
  files: any[];
  total_size: number;
  video_count: number;
  photo_count: number;
  audio_count: number;
  other_count: number;
}

interface MountedVolume {
  path: string;
  name: string;
  is_removable: boolean;
  total_space: number;
  free_space: number;
}


export default function SourcesPage() {
  const { sources, addSource, removeSource, isScanning, setIsScanning, nextStep } = useStore();
  const [error, setError] = useState<string | null>(null);
  const [detecting, setDetecting] = useState(false);
  const [showVolumes, setShowVolumes] = useState(false);
  const [volumes, setVolumes] = useState<MountedVolume[]>([]);

  const handleAddSource = async () => {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: 'Select source folder or volume',
      });

      if (selected) {
        await scanSource(selected as string);
      }
    } catch (e) {
      setError(`Failed to open folder picker: ${e}`);
    }
  };

  const scanSource = async (path: string) => {
    setIsScanning(true);
    setError(null);

    const label = path.split(/[\\/]/).pop() || 'Unknown';

    const source: Source = {
      path,
      label,
      exclusions: [],
    };

    try {
      const result = await invoke<SourceScanResult>('scan_source', { source });
      
      addSource({
        ...source,
        fileCount: result.files.length,
        totalSize: result.total_size,
        videos: result.video_count,
        photos: result.photo_count,
        audio: result.audio_count,
      });
    } catch (e) {
      setError(`Failed to scan: ${e}`);
      addSource(source);
    } finally {
      setIsScanning(false);
    }
  };

  const handleDetectVolumes = async () => {
    setDetecting(true);
    setError(null);
    setShowVolumes(true);
    
    try {
      const result = await invoke<MountedVolume[]>('get_mounted_volumes');
      // Filter to show only removable/volume-type drives
      const removableVolumes = result.filter(v => v.is_removable || v.path.startsWith('/Volumes/'));
      setVolumes(removableVolumes);
    } catch (e) {
      setError(`Failed to detect volumes: ${e}`);
    } finally {
      setDetecting(false);
    }
  };

  const handleAddVolume = async (volume: MountedVolume) => {
    // Dismiss first: scanning a card takes seconds, and awaiting it here left
    // the modal sitting open and unresponsive with the page's own scanning
    // spinner hidden behind it.
    setShowVolumes(false);
    setVolumes([]);
    await scanSource(volume.path);
  };

  const canProceed = sources.length > 0;

  return (
    <div className="max-w-3xl mx-auto">
      <div className="mb-6">
        <h1 className="text-2xl font-bold text-gray-900 dark:text-white">Sources</h1>
        <p className="text-gray-600 dark:text-gray-400 mt-1">
          Add folders, SD cards, or drives containing your media files
        </p>
      </div>

      {error && (
        <div className="mb-4 p-3 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg text-red-700 dark:text-red-400 text-sm">
          {error}
        </div>
      )}

      <div className="space-y-3 mb-6">
        {sources.map((source) => (
          <div key={source.path} className="card p-4 flex items-center gap-4">
            <div className="w-10 h-10 rounded-lg bg-accent/10 flex items-center justify-center">
              <svg className="w-5 h-5 text-accent" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
              </svg>
            </div>
            
            <div className="flex-1 min-w-0">
              <p className="font-medium text-gray-900 dark:text-white truncate">{source.label}</p>
              <p className="text-sm text-gray-500 dark:text-gray-400 font-mono truncate" title={source.path}>
                {source.path}
              </p>
              {source.fileCount !== undefined && (
                <div className="flex gap-3 mt-1 text-xs text-gray-500 dark:text-gray-400">
                  <span>{source.fileCount} files</span>
                  {source.videos ? <span>{source.videos} videos</span> : null}
                  {source.photos ? <span>{source.photos} photos</span> : null}
                  {source.audio ? <span>{source.audio} audio</span> : null}
                  {source.totalSize && <span>{formatSize(source.totalSize)}</span>}
                </div>
              )}
            </div>

            <button
              onClick={() => removeSource(source.path)}
              className="p-2 rounded-lg hover:bg-gray-100 dark:hover:bg-slate-700 text-gray-500"
            >
              <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
              </svg>
            </button>
          </div>
        ))}

        {sources.length === 0 && !isScanning && (
          <div className="card p-8 text-center border-dashed">
            <div className="w-16 h-16 mx-auto mb-4 rounded-full bg-gray-100 dark:bg-slate-800 flex items-center justify-center">
              <svg className="w-8 h-8 text-gray-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 9v3m0 0v3m0-3h3m-3 0H9m12 0a9 9 0 11-18 0 9 9 0 0118 0z" />
              </svg>
            </div>
            <p className="text-gray-600 dark:text-gray-400 mb-4">No sources added yet</p>
            <div className="flex gap-3 justify-center">
              <button onClick={handleAddSource} className="btn btn-primary">
                Add Source
              </button>
              <button onClick={handleDetectVolumes} className="btn btn-secondary">
                {detecting ? 'Detecting...' : 'Detect SD Cards'}
              </button>
            </div>
          </div>
        )}

        {isScanning && (
          <div className="card p-8 text-center">
            <div className="w-8 h-8 mx-auto mb-4 border-2 border-accent border-t-transparent rounded-full animate-spin" />
            <p className="text-gray-600 dark:text-gray-400">Scanning for media files...</p>
          </div>
        )}
      </div>

      {/* Detected Volumes Modal */}
      {showVolumes && volumes.length > 0 && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
          <div className="card w-full max-w-md p-6">
            <h2 className="text-xl font-semibold text-gray-900 dark:text-white mb-4">Detected Volumes</h2>
            <div className="space-y-2 mb-4 max-h-64 overflow-y-auto">
              {volumes.map((volume) => (
                <button
                  key={volume.path}
                  onClick={() => handleAddVolume(volume)}
                  className="w-full p-3 text-left rounded-lg border border-gray-200 dark:border-slate-700 hover:bg-gray-50 dark:hover:bg-slate-800"
                >
                  <div className="flex items-center justify-between">
                    <div>
                      <p className="font-medium text-gray-900 dark:text-white">{volume.name}</p>
                      <p className="text-sm text-gray-500 dark:text-gray-400 font-mono">{volume.path}</p>
                    </div>
                    <div className="text-right text-sm text-gray-500">
                      <p>{formatSize(volume.free_space)} free</p>
                    </div>
                  </div>
                </button>
              ))}
            </div>
            <div className="flex justify-end">
              <button onClick={() => { setShowVolumes(false); setVolumes([]); }} className="btn btn-secondary">
                Cancel
              </button>
            </div>
          </div>
        </div>
      )}

      {showVolumes && volumes.length === 0 && !detecting && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
          <div className="card w-full max-w-md p-6">
            <h2 className="text-xl font-semibold text-gray-900 dark:text-white mb-4">No SD Cards Found</h2>
            <p className="text-gray-600 dark:text-gray-400 mb-4">
              No removable volumes were detected. Make sure your SD card or USB drive is connected.
            </p>
            <div className="flex justify-end">
              <button onClick={() => { setShowVolumes(false); setVolumes([]); }} className="btn btn-secondary">
                Close
              </button>
            </div>
          </div>
        </div>
      )}

      {sources.length > 0 && (
        <div className="flex gap-3 mb-6">
          <button onClick={handleAddSource} className="btn btn-secondary">
            + Add Another Source
          </button>
          <button onClick={handleDetectVolumes} className="btn btn-secondary">
            Detect SD Cards
          </button>
        </div>
      )}

      <div className="flex justify-end">
        <button
          onClick={nextStep}
          disabled={!canProceed}
          className="btn btn-primary disabled:opacity-50 disabled:cursor-not-allowed"
        >
          Continue
        </button>
      </div>
    </div>
  );
}
