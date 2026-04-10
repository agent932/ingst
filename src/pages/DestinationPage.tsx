import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { useStore } from '../store/useStore';
import { formatSize } from '../utils/formatters';

interface DestInfo {
  path: string;
  exists: boolean;
  writable: boolean;
  free_space: number;
}


export default function DestinationPage() {
  const { destination, setDestination, nextStep, prevStep } = useStore();
  const [destInfo, setDestInfo] = useState<DestInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isValidating, setIsValidating] = useState(false);

  useEffect(() => {
    if (destination) {
      validateDestination(destination);
    }
  }, [destination]);

  const validateDestination = async (path: string) => {
    setIsValidating(true);
    setError(null);
    try {
      const info = await invoke<DestInfo>('get_dest_info', { path });
      setDestInfo(info);
      
      if (!info.exists) {
        setError('This folder does not exist. It will be created when you start the ingest.');
      } else if (!info.writable) {
        setError('This folder is not writable. Please choose a different location.');
      }
    } catch (e) {
      setError(`Failed to validate: ${e}`);
    } finally {
      setIsValidating(false);
    }
  };

  const handleSelectFolder = async () => {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: 'Select library destination',
        defaultPath: destination || undefined,
      });

      if (selected) {
        setDestination(selected as string);
      }
    } catch (e) {
      setError(`Failed to open folder picker: ${e}`);
    }
  };

  const canProceed = destination && destInfo?.writable;

  return (
    <div className="max-w-3xl mx-auto">
      <div className="mb-6">
        <h1 className="text-2xl font-bold text-gray-900 dark:text-white">Library Destination</h1>
        <p className="text-gray-600 dark:text-gray-400 mt-1">
          Choose where to organize your media files
        </p>
      </div>

      <div className="card p-6 mb-6">
        <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
          Library Folder
        </label>
        
        <div className="flex gap-3">
          <input
            type="text"
            value={destination}
            onChange={(e) => setDestination(e.target.value)}
            placeholder="Select or enter a folder path..."
            className="input flex-1 font-mono"
          />
          <button onClick={handleSelectFolder} className="btn btn-secondary">
            Browse
          </button>
        </div>

        {destInfo && (
          <div className="mt-4 p-4 bg-gray-50 dark:bg-slate-800 rounded-lg">
            <div className="grid grid-cols-3 gap-4 text-sm">
              <div>
                <p className="text-gray-500 dark:text-gray-400">Status</p>
                <p className={`font-medium ${destInfo.writable ? 'text-green-600 dark:text-green-400' : 'text-red-600 dark:text-red-400'}`}>
                  {destInfo.writable ? 'Writable' : 'Not writable'}
                </p>
              </div>
              <div>
                <p className="text-gray-500 dark:text-gray-400">Exists</p>
                <p className="font-medium text-gray-900 dark:text-white">
                  {destInfo.exists ? 'Yes' : 'Will be created'}
                </p>
              </div>
              <div>
                <p className="text-gray-500 dark:text-gray-400">Free Space</p>
                <p className="font-medium text-gray-900 dark:text-white">
                  {formatSize(destInfo.free_space)}
                </p>
              </div>
            </div>
          </div>
        )}

        {isValidating && (
          <div className="mt-4 flex items-center gap-2 text-sm text-gray-500">
            <div className="w-4 h-4 border-2 border-accent border-t-transparent rounded-full animate-spin" />
            Validating...
          </div>
        )}

        {error && (
          <div className={`mt-4 p-3 rounded-lg text-sm ${
            error.includes('not writable')
              ? 'bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 text-red-700 dark:text-red-400'
              : 'bg-yellow-50 dark:bg-yellow-900/20 border border-yellow-200 dark:border-yellow-800 text-yellow-700 dark:text-yellow-400'
          }`}>
            {error}
          </div>
        )}
      </div>

      <div className="card p-6 mb-6">
        <h3 className="font-medium text-gray-900 dark:text-white mb-3">Preview Folder Structure</h3>
        <div className="font-mono text-sm text-gray-600 dark:text-gray-400 bg-gray-50 dark:bg-slate-800 p-4 rounded-lg">
          {destination || '/path/to/library'}/<span className="text-accent">YYYY</span>/<span className="text-accent">MM</span>/<span className="text-accent">DEVICE_NAME</span>/
          <br />│
          <br />├── 2024/
          <br />│   └── 01/
          <br />│       └── iPhone15/
          <br />│           └── video.mp4
          <br />├── 2024/
          <br />│   └── 03/
          <br />│       └── SonyA7IV/
          <br />│           └── photo.arw
        </div>
        <p className="mt-3 text-xs text-gray-500 dark:text-gray-400">
          Files are organized by capture year, month, and device name
        </p>
      </div>

      <div className="flex justify-between">
        <button onClick={prevStep} className="btn btn-secondary">
          Back
        </button>
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
