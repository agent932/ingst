import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';

export interface Source {
  path: string;
  label: string;
  exclusions: string[];
  fileCount?: number;
  totalSize?: number;
  videos?: number;
  photos?: number;
  audio?: number;
}

export interface ScannedFile {
  path: string;
  name: string;
  size: number;
  modified: string;
  created?: string;
  capture_date?: string;
  device_name?: string;
  file_type: string;
  hash?: string;
}

export interface IngestOperation {
  source_path: string;
  dest_path: string;
  action: string;
  size: number;
  capture_date?: string;
  device_name: string;
  hash?: string;
}

export interface IngestPlan {
  operations: IngestOperation[];
  total_files: number;
  total_size: number;
  duplicate_count: number;
}

export interface ProgressEvent {
  current_file: string;
  current_index: number;
  total: number;
  bytes_copied: number;
  total_bytes: number;
  current_file_bytes: number;
  current_file_total: number;
  elapsed_secs: number;
  status: string;
}

export interface Settings {
  last_destination?: string;
  default_operation: string;
  skip_duplicates_default: boolean;
  theme: string;
}

export interface IngestResult {
  success_count: number;
  skipped_count: number;
  error_count: number;
  errors: string[];
  log_path: string;
  cancelled: boolean;
  remaining_count: number;
}

interface StoreState {
  currentStep: number;
  theme: 'light' | 'dark' | 'system';
  sources: Source[];
  destination: string;
  operation: 'copy' | 'move';
  skipDuplicates: boolean;
  scannedFiles: ScannedFile[];
  ingestPlan: IngestPlan | null;
  progress: ProgressEvent | null;
  ingestResult: IngestResult | null;
  isScanning: boolean;
  isIngesting: boolean;
  /** Whether the current plan's transfer has already been launched. */
  ingestStarted: boolean;
  settings: Settings;

  setCurrentStep: (step: number) => void;
  setTheme: (theme: 'light' | 'dark' | 'system') => void;
  addSource: (source: Source) => void;
  removeSource: (path: string) => void;
  updateSource: (path: string, updates: Partial<Source>) => void;
  setDestination: (path: string) => void;
  setOperation: (op: 'copy' | 'move') => void;
  setSkipDuplicates: (skip: boolean) => void;
  setScannedFiles: (files: ScannedFile[]) => void;
  setIngestPlan: (plan: IngestPlan | null) => void;
  setProgress: (progress: ProgressEvent | null) => void;
  setIngestResult: (result: IngestResult | null) => void;
  setIsScanning: (scanning: boolean) => void;
  setIsIngesting: (ingesting: boolean) => void;
  setIngestStarted: (started: boolean) => void;
  loadSettings: () => Promise<void>;
  saveSettings: () => Promise<void>;
  nextStep: () => void;
  prevStep: () => void;
  reset: () => void;
}

export const useStore = create<StoreState>((set, get) => ({
  currentStep: 1,
  theme: 'system',
  sources: [],
  destination: '',
  operation: 'copy',
  skipDuplicates: true,
  scannedFiles: [],
  ingestPlan: null,
  progress: null,
  ingestResult: null,
  isScanning: false,
  isIngesting: false,
  ingestStarted: false,
  settings: {
    default_operation: 'copy',
    skip_duplicates_default: true,
    theme: 'system',
  },

  setCurrentStep: (step) => set({ currentStep: step }),
  setTheme: (theme) => set({ theme }),
  
  addSource: (source) => set((state) => ({ 
    sources: [...state.sources, source] 
  })),
  
  removeSource: (path) => set((state) => ({ 
    sources: state.sources.filter(s => s.path !== path) 
  })),
  
  updateSource: (path, updates) => set((state) => ({
    sources: state.sources.map(s => 
      s.path === path ? { ...s, ...updates } : s
    )
  })),
  
  setDestination: (path) => set({ destination: path }),
  setOperation: (op) => set({ operation: op }),
  setSkipDuplicates: (skip) => set({ skipDuplicates: skip }),
  setScannedFiles: (files) => set({ scannedFiles: files }),

  // A new plan means a new run: clear the previous run's progress, result, and
  // started flag so the Ingest step can execute again.
  setIngestPlan: (plan) => set({
    ingestPlan: plan,
    ingestStarted: false,
    ingestResult: null,
    progress: null,
  }),

  setProgress: (progress) => set({ progress }),
  setIngestResult: (result) => set({ ingestResult: result }),
  setIsScanning: (scanning) => set({ isScanning: scanning }),
  setIsIngesting: (ingesting) => set({ isIngesting: ingesting }),
  setIngestStarted: (started) => set({ ingestStarted: started }),
  
  loadSettings: async () => {
    try {
      const settings = await invoke<Settings>('get_settings');
      set({ 
        settings,
        theme: settings.theme as 'light' | 'dark' | 'system',
        destination: settings.last_destination || '',
        operation: settings.default_operation as 'copy' | 'move',
        skipDuplicates: settings.skip_duplicates_default,
      });
    } catch (e) {
      console.error('Failed to load settings:', e);
    }
  },
  
  saveSettings: async () => {
    const { theme, destination, operation, skipDuplicates } = get();
    const settings: Settings = {
      last_destination: destination,
      default_operation: operation,
      skip_duplicates_default: skipDuplicates,
      theme,
    };
    try {
      await invoke('save_settings', { settings });
    } catch (e) {
      console.error('Failed to save settings:', e);
    }
  },
  
  nextStep: () => set((state) => ({ 
    currentStep: Math.min(state.currentStep + 1, 6) 
  })),
  
  prevStep: () => set((state) => ({ 
    currentStep: Math.max(state.currentStep - 1, 1) 
  })),
  
  reset: () => set((state) => ({
    currentStep: 1,
    sources: [],
    destination: '',
    // Fall back to the user's saved defaults rather than hardcoded values,
    // which used to silently undo their Settings choices on every new ingest.
    operation: (state.settings.default_operation as 'copy' | 'move') || 'copy',
    skipDuplicates: state.settings.skip_duplicates_default,
    scannedFiles: [],
    ingestPlan: null,
    progress: null,
    ingestResult: null,
    ingestStarted: false,
  })),
}));
