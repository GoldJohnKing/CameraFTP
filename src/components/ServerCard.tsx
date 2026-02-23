import { Power, Loader2 } from 'lucide-react';
import { useState, useCallback } from 'react';
import { toast } from 'sonner';
import { useServerStore } from '../stores/serverStore';
import { useStoragePermission } from '../hooks/useStoragePermission';


export function ServerCard() {
  const [isStarting, setIsStarting] = useState(false);
  const { 
    isRunning, 
    isLoading, 
    error,
    startServer, 
    stopServer 
  } = useServerStore();
  const { 
    needsPermission, 
    checkPrerequisites, 
    requestAllFilesPermission, 
    ensureStorageReady 
  } = useStoragePermission();

  const handleStartServer = useCallback(async () => {
    if (isStarting) return;
    
    setIsStarting(true);
    
    try {
      // 1. Check prerequisites
      const check = await checkPrerequisites();
      
      if (!check.can_start) {
        if (needsPermission) {
          // 需要权限 - 显示原因并直接跳转到设置页面
          toast.error('需要"所有文件访问权限"才能启动服务器，即将跳转到设置');
          await requestAllFilesPermission(false); // 跳转到设置，不再显示第二个 toast
          setIsStarting(false);
          return;
        }
        
        // 尝试创建目录
        const result = await ensureStorageReady();
        if (!result.success) {
          setIsStarting(false);
          return;
        }
      }
      
      // 启动服务器
      await startServer();
      toast.success('FTP服务器已启动');
      
    } catch (err) {
      console.error('Failed to start server:', err);
      toast.error('启动服务器失败：' + (err instanceof Error ? err.message : String(err)));
    } finally {
      setIsStarting(false);
    }
  }, [isStarting, checkPrerequisites, needsPermission, requestAllFilesPermission, ensureStorageReady, startServer]);

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
        disabled={isLoading || isStarting}
        className={`w-full py-3 px-4 rounded-lg font-medium flex items-center justify-center gap-2 transition-colors ${
          isRunning
            ? 'bg-red-50 text-red-600 hover:bg-red-100'
            : 'bg-blue-600 text-white hover:bg-blue-700'
        } disabled:opacity-50 disabled:cursor-not-allowed`}
      >
        {isLoading || isStarting ? (
          <Loader2 className="w-5 h-5 animate-spin" />
        ) : (
          <Power className="w-5 h-5" />
        )}
        {isRunning 
          ? '停止服务器' 
          : isStarting 
            ? '启动中...' 
            : '启动服务器'
        }
      </button>

      {error && (
        <p className="mt-3 text-sm text-red-600 text-center">{error}</p>
      )}
    </div>
  );
}
