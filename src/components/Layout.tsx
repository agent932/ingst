import { useState } from 'react';
import { useStore } from '../store/useStore';
import SettingsModal from './SettingsModal';

const steps = [
  { number: 1, label: 'Sources', description: 'Add media sources' },
  { number: 2, label: 'Destination', description: 'Choose library folder' },
  { number: 3, label: 'Organize', description: 'Ingest options' },
  { number: 4, label: 'Review', description: 'Preview plan' },
  { number: 5, label: 'Ingest', description: 'Copy or move files' },
  { number: 6, label: 'Summary', description: 'Complete' },
];

export default function Layout({ children }: { children: React.ReactNode }) {
  const { currentStep, setCurrentStep, theme, setTheme, sources, destination, ingestPlan, ingestResult, isIngesting } = useStore();
  const [showSettings, setShowSettings] = useState(false);

  const hasSources = sources.length > 0;
  const hasDest = destination !== '';
  const hasPlan = ingestPlan !== null;
  const hasResult = ingestResult !== null;

  // A step is reachable if its prerequisites are met OR the user has already been there.
  function isStepReachable(stepNumber: number): boolean {
    // Pin the user to the Ingest step while files are in flight — leaving it
    // would hide a transfer that is still running in the background.
    if (isIngesting) {
      return stepNumber === currentStep;
    }

    switch (stepNumber) {
      case 1: return true;
      case 2: return hasSources;
      case 3: return hasSources && hasDest;
      case 4: return hasSources && hasDest;
      case 5: return hasPlan;
      case 6: return hasResult;
      default: return false;
    }
  }

  return (
    <div className="flex flex-col h-full bg-background-light dark:bg-background-dark">
      <header className="h-14 flex items-center justify-between px-6 border-b border-gray-200 dark:border-slate-700 bg-white dark:bg-slate-900">
        <div className="flex items-center gap-3">
          {/* Same geometry as src-tauri/icons/icon.svg, so the mark in the
              window matches the one in the Dock. Was a cloud-upload glyph,
              which told a first-time user their footage was going to a
              service — the opposite of what this app does. */}
          {/* Bare mark, no tile. The tile in src-tauri/icons/icon.svg exists so
              the icon survives an arbitrary background in the Dock or Finder;
              in here the header already provides that, and a near-black tile on
              a near-black bar just dissolves. Geometry is otherwise identical. */}
          <svg className="h-7 w-auto" viewBox="11 8 42 50" aria-hidden="true">
            <path d="M24 10 H40 V25 H49 L32 41 L15 25 H24 Z" fill="#2DD4BF" />
            <rect x="13" y="47" width="38" height="9" rx="4.5" fill="#64748B" />
          </svg>
          <span className="text-lg font-semibold text-gray-900 dark:text-white">Ingst</span>
        </div>
        
        <div className="flex items-center gap-2">
          <button
            onClick={() => setTheme(theme === 'dark' ? 'light' : theme === 'light' ? 'system' : 'dark')}
            className="p-2 rounded-lg hover:bg-gray-100 dark:hover:bg-slate-800 text-gray-600 dark:text-gray-300"
            title={`Theme: ${theme}`}
            aria-label={`Switch theme (current: ${theme})`}
          >
            {theme === 'dark' ? (
              <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M20.354 15.354A9 9 0 018.646 3.646 9.003 9.003 0 0012 21a9.003 9.003 0 008.354-5.646z" />
              </svg>
            ) : theme === 'light' ? (
              <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 3v1m0 16v1m9-9h-1M4 12H3m15.364 6.364l-.707-.707M6.343 6.343l-.707-.707m12.728 0l-.707.707M6.343 17.657l-.707.707M16 12a4 4 0 11-8 0 4 4 0 018 0z" />
              </svg>
            ) : (
              <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9.75 17L9 20l-1 1h8l-1-1-.75-3M3 13h18M5 17h14a2 2 0 002-2V5a2 2 0 00-2-2H5a2 2 0 00-2 2v10a2 2 0 002 2z" />
              </svg>
            )}
          </button>
          
          <button
            onClick={() => setShowSettings(true)}
            className="p-2 rounded-lg hover:bg-gray-100 dark:hover:bg-slate-800 text-gray-600 dark:text-gray-300"
            title="Settings"
            aria-label="Open settings"
          >
            <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z" />
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
            </svg>
          </button>
        </div>
      </header>
      
      <div className="flex flex-1 overflow-hidden">
        <nav className="w-56 border-r border-gray-200 dark:border-slate-700 bg-white dark:bg-slate-900 p-4">
          <ol className="space-y-2">
            {steps.map((step) => (
              <li key={step.number}>
                <button
                  onClick={() => isStepReachable(step.number) && setCurrentStep(step.number)}
                  disabled={!isStepReachable(step.number)}
                  className={`step-item w-full text-left ${
                    !isStepReachable(step.number)
                      ? 'opacity-40 cursor-not-allowed'
                      : currentStep === step.number
                      ? 'step-item-active'
                      : currentStep > step.number
                      ? 'step-item-complete'
                      : 'step-item-inactive'
                  }`}
                >
                  <div className={`w-6 h-6 rounded-full flex items-center justify-center text-xs font-medium ${
                    currentStep === step.number
                      ? 'bg-accent text-white'
                      : currentStep > step.number
                      ? 'bg-green-500 text-white'
                      : 'bg-gray-200 dark:bg-slate-700 text-gray-500 dark:text-gray-400'
                  }`}>
                    {currentStep > step.number ? (
                      <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
                      </svg>
                    ) : (
                      step.number
                    )}
                  </div>
                  <div className="flex-1 min-w-0">
                    <p className="text-sm font-medium truncate">{step.label}</p>
                    <p className="text-xs text-gray-500 dark:text-gray-400 truncate">{step.description}</p>
                  </div>
                </button>
              </li>
            ))}
          </ol>
        </nav>
        
        <main className="flex-1 overflow-auto p-6">
          {children}
        </main>
      </div>
      
      {showSettings && <SettingsModal onClose={() => setShowSettings(false)} />}
    </div>
  );
}
