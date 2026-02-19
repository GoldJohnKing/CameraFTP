import { useState } from 'react';
import { ServerCard } from './components/ServerCard';
import { StatsCard } from './components/StatsCard';
import { InfoCard } from './components/InfoCard';
import { ServerInfo } from './types';
import { Camera } from 'lucide-react';

function App() {
  const [serverInfo, setServerInfo] = useState<ServerInfo | null>(null);

  return (
    <div className="min-h-screen bg-gray-50">
      <div className="max-w-md mx-auto p-4">
        {/* Header */}
        <header className="text-center py-6">
          <div className="w-16 h-16 bg-blue-600 rounded-2xl flex items-center justify-center mx-auto mb-3">
            <Camera className="w-8 h-8 text-white" />
          </div>
          <h1 className="text-2xl font-bold text-gray-900">
            图传伴侣
          </h1>
          <p className="text-sm text-gray-500 mt-1">
            Camera FTP Companion
          </p>
        </header>

        {/* Main Content */}
        <div className="space-y-4">
          <ServerCard onStatusChange={setServerInfo} />
          <StatsCard />
          <InfoCard serverInfo={serverInfo} />
        </div>

        {/* Footer */}
        <footer className="text-center py-6 text-xs text-gray-400">
          <p>© 2025 Camera FTP Companion</p>
          <p className="mt-1">让摄影工作流更简单</p>
        </footer>
      </div>
    </div>
  );
}

export default App;