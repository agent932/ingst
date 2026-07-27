import { useEffect } from 'react';
import { useStore } from './store/useStore';
import Layout from './components/Layout';
import SourcesPage from './pages/SourcesPage';
import DestinationPage from './pages/DestinationPage';
import OrganizePage from './pages/OrganizePage';
import ReviewPage from './pages/ReviewPage';
import IngestPage from './pages/IngestPage';
import SummaryPage from './pages/SummaryPage';

function App() {
  const { currentStep, theme, loadSettings } = useStore();

  // Settings were saved but never read back, so theme, default operation and
  // last destination were all discarded on every launch.
  useEffect(() => {
    loadSettings();
  }, []);

  useEffect(() => {
    const root = window.document.documentElement;
    if (theme === 'dark') {
      root.classList.add('dark');
    } else if (theme === 'light') {
      root.classList.remove('dark');
    } else {
      const isDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
      if (isDark) {
        root.classList.add('dark');
      } else {
        root.classList.remove('dark');
      }
    }
  }, [theme]);

  const renderStep = () => {
    switch (currentStep) {
      case 1:
        return <SourcesPage />;
      case 2:
        return <DestinationPage />;
      case 3:
        return <OrganizePage />;
      case 4:
        return <ReviewPage />;
      case 5:
        return <IngestPage />;
      case 6:
        return <SummaryPage />;
      default:
        return <SourcesPage />;
    }
  };

  return <Layout>{renderStep()}</Layout>;
}

export default App;
