/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import wechatQrCodeSrc from '../assets/donate-qrcode-wechat.png';
import { Dialog } from './ui';

interface WeChatDonateDialogProps {
  isOpen: boolean;
  onClose: () => void;
}

export function WeChatDonateDialog({
  isOpen,
  onClose,
}: WeChatDonateDialogProps) {
  return (
    <Dialog
      isOpen={isOpen}
      onClose={onClose}
      title="微信收款"
      zIndex="z-[60]"
      overlayClassName="bg-black/70"
      data-testid="wechat-donate-dialog-overlay"
      footer={
        <button
          onClick={onClose}
          className="px-4 py-2 bg-gray-100 text-gray-700 rounded-lg hover:bg-gray-200 transition-colors text-sm font-medium"
        >
          关闭
        </button>
      }
    >
      <div
        data-testid="wechat-donate-dialog-content"
        className="flex flex-col items-center gap-4"
      >
        <div className="bg-white rounded-xl p-2 border border-gray-200">
          <img src={wechatQrCodeSrc} alt="微信收款码" className="w-72 h-auto" />
        </div>

        <p className="text-sm text-gray-600 text-left leading-6">
          请先对当前界面截图，然后打开微信扫一扫，识别截图中的收款码。
        </p>
      </div>
    </Dialog>
  );
}
