/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { getVersion } from '@tauri-apps/api/app';
import { createPortal } from 'react-dom';
import { Info, X, ExternalLink, ChevronDown, ChevronUp } from 'lucide-react';
import { Card, CardHeader } from './ui';

async function openExternalLink(url: string) {
  // Android 平台：使用 JS Bridge
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const permissionAndroid = (window as unknown as Record<string, unknown>).PermissionAndroid;
  if (permissionAndroid && typeof permissionAndroid === 'object') {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const openLink = (permissionAndroid as Record<string, unknown>).openExternalLink;
    if (typeof openLink === 'function') {
      (openLink as (url: string) => void)(url);
      return;
    }
  }

  try {
    await invoke('open_external_link', { url });
  } catch (err) {
    console.warn('Failed to open external link:', err);
    // Fallback: try to open via window.open (may not work in Tauri)
    window.open(url, '_blank');
  }
}

interface Dependency {
  name: string;
  description: string;
  url: string;
}

interface DependencyGroup {
  title: string;
  deps: Dependency[];
}

const DEPENDENCIES: DependencyGroup[] = [
  {
    title: '应用框架',
    deps: [
      {
        name: 'Tauri',
        description: '使用 Web 前端构建桌面应用的框架',
        url: 'https://tauri.app/',
      },
      {
        name: 'React',
        description: '用于构建用户界面的 JavaScript 库',
        url: 'https://react.dev/',
      },
      {
        name: 'TailwindCSS',
        description: '实用优先的 CSS 框架',
        url: 'https://tailwindcss.com/',
      },
    ],
  },
  {
    title: 'FTP 服务器',
    deps: [
      {
        name: 'libunftp',
        description: 'Rust 编写的异步 FTP 服务器库',
        url: 'https://docs.rs/libunftp/',
      },
      {
        name: 'unftp-sbe-fs',
        description: 'libunftp 的文件系统存储后端',
        url: 'https://docs.rs/unftp-sbe-fs/',
      },
      {
        name: 'tokio',
        description: 'Rust 异步运行时',
        url: 'https://tokio.rs/',
      },
    ],
  },
  {
    title: '图像与文件处理',
    deps: [
      {
        name: 'nom-exif',
        description: 'EXIF 元数据解析库（支持 JPG/RAW/CR3/NEF 等）',
        url: 'https://docs.rs/nom-exif/',
      },
      {
        name: 'image',
        description: 'Rust 图像处理库',
        url: 'https://docs.rs/image/',
      },
      {
        name: 'notify',
        description: '跨平台文件系统事件监听库',
        url: 'https://docs.rs/notify/',
      },
    ],
  },
  {
    title: '安全与工具',
    deps: [
      {
        name: 'Argon2',
        description: '密码哈希算法实现',
        url: 'https://docs.rs/argon2/',
      },
      {
        name: 'zeroize',
        description: '内存安全：敏感数据自动清零',
        url: 'https://docs.rs/zeroize/',
      },
      {
        name: 'chrono',
        description: '日期和时间处理库',
        url: 'https://docs.rs/chrono/',
      },
    ],
  },
];

// 关于对话框内容组件
interface AboutDialogProps {
  isOpen: boolean;
  onClose: () => void;
}

function AboutDialog({ isOpen, onClose }: AboutDialogProps) {
  const [version, setVersion] = useState<string>('0.0.0');
  const [expandedGroups, setExpandedGroups] = useState<Set<string>>(new Set());

  useEffect(() => {
    if (isOpen) {
      getVersion().then(setVersion).catch(() => setVersion('0.0.0'));
    }
  }, [isOpen]);

  const toggleGroup = (title: string) => {
    const newSet = new Set(expandedGroups);
    if (newSet.has(title)) {
      newSet.delete(title);
    } else {
      newSet.add(title);
    }
    setExpandedGroups(newSet);
  };

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-4">
      <div className="bg-white rounded-xl max-w-lg w-full max-h-[80vh] shadow-2xl flex flex-col">
        {/* 头部 */}
        <div className="flex items-center justify-between p-4 border-b">
          <div className="flex items-center gap-3">
            <div className="w-10 h-10 bg-blue-600 rounded-xl flex items-center justify-center">
              <Info className="w-5 h-5 text-white" />
            </div>
            <div>
              <h2 className="text-lg font-semibold text-gray-900">关于图传伴侣</h2>
              <p className="text-sm text-gray-500">版本 {version}</p>
            </div>
          </div>
          <button
            onClick={onClose}
            className="p-2 text-gray-400 hover:text-gray-600 hover:bg-gray-100 rounded-lg transition-colors"
            aria-label="关闭"
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        {/* 内容区域 */}
        <div className="overflow-y-auto p-4 space-y-6">
          {/* 项目简介 */}
          <div>
            <h3 className="text-sm font-semibold text-gray-900 mb-3">项目简介</h3>
            <div className="bg-gray-50 rounded-lg p-4">
              <p className="text-sm text-gray-700 leading-relaxed">
                CameraFTP 是一款跨平台的 FTP 文件传输工具，专为摄影师设计。让您可以方便地将相机中的照片通过 FTP 协议无线传输到电脑或移动设备。
              </p>
            </div>
          </div>

          {/* 项目地址 */}
          <div>
            <h3 className="text-sm font-semibold text-gray-900 mb-3">项目地址</h3>
            <div className="bg-gray-50 rounded-lg p-4">
              <div className="flex items-center gap-1">
                <svg className="w-4 h-4 flex-shrink-0" viewBox="0 0 24 24" fill="currentColor">
                  <path d="M12 0c-6.626 0-12 5.373-12 12 0 5.302 3.438 9.8 8.207 11.387.599.111.793-.261.793-.577v-2.234c-3.338.726-4.033-1.416-4.033-1.416-.546-1.387-1.333-1.756-1.333-1.756-1.089-.745.083-.729.083-.729 1.205.084 1.839 1.237 1.839 1.237 1.07 1.834 2.807 1.304 3.492.997.107-.775.418-1.305.762-1.604-2.665-.305-5.467-1.334-5.467-5.931 0-1.311.469-2.381 1.236-3.221-.124-.303-.535-1.524.117-3.176 0 0 1.008-.322 3.301 1.23.957-.266 1.983-.399 3.003-.404 1.02.005 2.047.138 3.006.404 2.291-1.552 3.297-1.23 3.297-1.23.653 1.653.242 2.874.118 3.176.77.84 1.235 1.911 1.235 3.221 0 4.609-2.807 5.624-5.479 5.921.43.372.823 1.102.823 2.222v3.293c0 .319.192.694.801.576 4.765-1.589 8.199-6.086 8.199-11.386 0-6.627-5.373-12-12-12z" />
                </svg>
                <button
                  onClick={() => openExternalLink('https://github.com/GoldJohnKing/CameraFTP')}
                  className="text-sm text-blue-600 hover:text-blue-700 inline-flex items-center gap-0.5"
                >
                  GoldJohnKing/CameraFTP
                  <ExternalLink className="w-3 h-3" />
                </button>
              </div>
            </div>
          </div>

          {/* 开源协议 */}
          <div>
            <h3 className="text-sm font-semibold text-gray-900 mb-3">开源协议</h3>
            <div className="bg-gray-50 rounded-lg p-4">
              <p className="text-sm text-gray-700">
                本软件采用{' '}
                <button
                  onClick={() => openExternalLink('https://www.gnu.org/licenses/agpl-3.0.html')}
                  className="text-blue-600 hover:text-blue-700 inline-flex items-center gap-0.5"
                >
                  AGPL-3.0-or-later
                  <ExternalLink className="w-3 h-3" />
                </button>
                {' '}协议授权
              </p>
              <p className="text-sm text-gray-500 mt-1">
                Copyright © 2026{' '}
                <button
                  onClick={() => openExternalLink('https://github.com/GoldJohnKing')}
                  className="text-blue-600 hover:text-blue-700 inline-flex items-center gap-0.5"
                >
                  GoldJohnKing
                  <ExternalLink className="w-3 h-3" />
                </button>
              </p>
            </div>
          </div>

          {/* 使用的开源项目 */}
          <div>
            <h3 className="text-sm font-semibold text-gray-900 mb-3">使用的开源项目</h3>
            <div className="space-y-2">
              {DEPENDENCIES.map((group) => (
                <div key={group.title} className="border rounded-lg overflow-hidden">
                  <button
                    onClick={() => toggleGroup(group.title)}
                    className="w-full flex items-center justify-between p-3 bg-gray-50 hover:bg-gray-100 transition-colors"
                  >
                    <span className="text-sm font-medium text-gray-700">
                      {group.title}
                    </span>
                    {expandedGroups.has(group.title) ? (
                      <ChevronUp className="w-4 h-4 text-gray-500" />
                    ) : (
                      <ChevronDown className="w-4 h-4 text-gray-500" />
                    )}
                  </button>
                  {expandedGroups.has(group.title) && (
                    <div className="p-3 space-y-3 bg-white">
                      {group.deps.map((dep) => (
                        <div key={dep.name} className="flex flex-col gap-0.5">
                          <button
                            onClick={() => openExternalLink(dep.url)}
                            className="text-sm font-medium text-blue-600 hover:text-blue-700 inline-flex items-center gap-1 text-left break-all"
                          >
                            {dep.name}
                            <ExternalLink className="w-3 h-3 flex-shrink-0" />
                          </button>
                          <p className="text-xs text-gray-500">{dep.description}</p>
                        </div>
                      ))}
                    </div>
                  )}
                </div>
              ))}
            </div>
          </div>
        </div>

        {/* 底部按钮 */}
        <div className="border-t p-4 flex justify-end">
          <button
            onClick={onClose}
            className="px-4 py-2 bg-gray-100 text-gray-700 rounded-lg hover:bg-gray-200 transition-colors text-sm font-medium"
          >
            关闭
          </button>
        </div>
      </div>
    </div>
  );
}

export function AboutCard() {
  const [isOpen, setIsOpen] = useState(false);
  const [version, setVersion] = useState<string>('0.0.0');

  useEffect(() => {
    getVersion().then(setVersion).catch(() => setVersion('0.0.0'));
  }, []);

  return (
    <>
      {/* 关于卡片 */}
      <Card>
        <CardHeader
          title="关于"
          description="应用信息、开源协议与致谢"
          icon={<Info className="w-5 h-5 text-blue-600" />}
        />

        <div className="p-4">
          <button
            onClick={() => setIsOpen(true)}
            className="w-full text-left p-3 bg-gray-50 hover:bg-gray-100 rounded-lg transition-colors flex items-center justify-between group"
          >
            <div>
              <h4 className="font-medium text-gray-900">图传伴侣 CameraFTP</h4>
              <p className="text-sm text-gray-500 mt-0.5">版本 {version}</p>
            </div>
            <Info className="w-5 h-5 text-gray-400 group-hover:text-gray-600" />
          </button>
        </div>
      </Card>

      {/* 关于对话框 - 使用 Portal 渲染到 body 下 */}
      {createPortal(
        <AboutDialog isOpen={isOpen} onClose={() => setIsOpen(false)} />,
        document.body
      )}
    </>
  );
}
