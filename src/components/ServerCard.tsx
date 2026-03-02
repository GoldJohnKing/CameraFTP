import { Power } from 'lucide-react';
import { useState, useCallback, memo } from 'react';
import { toast } from 'sonner';
import { useServerStore } from '../stores/serverStore';
import { usePermissionStore } from '../stores/permissionStore';
import { LoadingButton, ErrorMessage } from './ui';
import { formatError } from '../utils/error';


export const ServerCard = memo(function ServerCard() {
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
  } = usePermissionStore();

  const handleStartServer = useCallback(async () => {
    if (isStarting) return;
    
    setIsStarting(true);
    
    try {
      // 1. Check prerequisites
      const check = await checkPrerequisites();
      
      if (!check.canStart) {
        if (needsPermission) {
          // 需要权限 - 直接跳转到设置页面
          await requestAllFilesPermission();
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
      // 不再显示Toast - 系统通知栏已提供状态提示
      
    } catch (err) {
      toast.error('启动服务器失败：' + formatError(err));
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
    } catch {
      // 错误已在 store 中处理
    }
  };

  return (
    <div>
      <LoadingButton
        isLoading={isLoading || isStarting}
        loadingText={isRunning ? '停止中...' : '启动中...'}
        onClick={handleToggle}
        variant={isRunning ? 'danger' : 'primary'}
        icon={<Power className="w-5 h-5" />}
      >
        {isRunning ? '停止服务器' : '启动服务器'}
      </LoadingButton>

      <ErrorMessage message={error} />
    </div>
  );
});
