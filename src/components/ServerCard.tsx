import { Power, Loader2 } from 'lucide-react';
import { useState, useCallback } from 'react';
import { toast } from 'sonner';
import { useServerStore } from '../stores/serverStore';
import { useStoragePermission } from '../hooks/useStoragePermission';
import { useSAFPicker } from '../hooks/useSAFPicker';

export function ServerCard() {
  const [isStarting, setIsStarting] = useState(false);
  const { 
    isRunning, 
    isLoading, 
    error,
    startServer, 
    stopServer 
  } = useServerStore();
  const { pathInfo, isChecking, checkPrerequisites, saveStoragePath, getLastUri } = useStoragePermission();
  const { openPicker } = useSAFPicker();

  const handleStartServer = useCallback(async () => {
    if (isStarting || isChecking) return;
    
    setIsStarting(true);
    
    try {
      // 1. Check prerequisites
      const check = await checkPrerequisites();
      
      if (!check.can_start) {
        // Show toast with reason
        const reason = check.reason || '需要配置存储路径';
        toast.info(`${reason}，请选择存储目录`);
        
        // 2. Get last URI for pre-selection
        const lastUri = await getLastUri();
        
        // 3. Open SAF picker
        const pickerResult = await openPicker(lastUri || undefined);
        
        if (!pickerResult) {
          // User cancelled
          toast.warning('未选择存储路径，服务器未启动');
          setIsStarting(false);
          return;
        }
        
        // 4. Save the selected path
        const saved = await saveStoragePath(pickerResult.name, pickerResult.uri);
        
        if (!saved) {
          toast.error('保存存储路径失败，请重试');
          setIsStarting(false);
          return;
        }
        
        // 5. Show success toast
        toast.success(`存储路径已设置为：${pickerResult.name}`);
      }
      
      // 6. Start the server
      await startServer();
      toast.success('FTP服务器已启动');
      
    } catch (err) {
      console.error('Failed to start server:', err);
      toast.error('启动服务器失败：' + (err instanceof Error ? err.message : String(err)));
    } finally {
      setIsStarting(false);
    }
  }, [isStarting, isChecking, checkPrerequisites, saveStoragePath, getLastUri, openPicker, startServer]);

  const handleToggle = async () => {
    try {
      if (isRunning) {
        await stopServer();
      } else {
        await handleStartServer();
      }
    } catch (err) {
      // 错误已在 store 中处理
      console.error('Server toggle failed:', err);
    }
  };

  return (
    <div>
      <button
        onClick={handleToggle}
        disabled={isLoading || isStarting || isChecking}
        className={`w-full py-3 px-4 rounded-lg font-medium flex items-center justify-center gap-2 transition-colors ${
          isRunning
            ? 'bg-red-50 text-red-600 hover:bg-red-100'
            : 'bg-blue-600 text-white hover:bg-blue-700'
        } disabled:opacity-50 disabled:cursor-not-allowed`}
      >
        {isLoading || isStarting || isChecking ? (
          <Loader2 className="w-5 h-5 animate-spin" />
        ) : (
          <Power className="w-5 h-5" />
        )}
        {isRunning 
          ? '停止服务器' 
          : isStarting 
            ? '启动中...' 
            : isChecking 
              ? '检查中...' 
              : '启动服务器'
        }
      </button>

      {error && (
        <p className="mt-3 text-sm text-red-600 text-center">{error}</p>
      )}

      {pathInfo && (
        <div className="text-sm text-gray-500 mt-2 text-center">
          存储路径：{pathInfo.path_name} {pathInfo.is_valid ? '✅' : '❌'}
        </div>
      )}
    </div>
  );
}
