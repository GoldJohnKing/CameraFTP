# Windows 10/11 原生光标设计规范研究报告

## 目录
1. [概述](#概述)
2. [抓手光标 (Grab/Grabbing)](#一抓手光标-grabgrabbing)
3. [工字光标 (Text/I-beam)](#二工字光标-texti-beam)
4. [动态加载方案分析](#三动态加载方案分析)
5. [参考资料](#参考资料)

---

## 概述

根据 Microsoft 官方文档，Windows 原生光标遵循以下设计原则：

### 光标尺寸规范
- **标准光标尺寸**：Windows 系统光标通常为 **32x32 像素**
- **热点 (Hot Spot)**：光标上精确定义的像素点，表示实际触发位置
- **所有指针（除了忙碌指针）都有一个单像素热点**定义鼠标的精确屏幕位置

### 颜色规范
- **描边颜色**：Windows 11 使用 Fluent Design 设计语言
- **描边粗细**：1px 轮廓线
- **填充颜色**：纯黑色 (#000000) + 白色描边边缘，确保在任何背景上都可见

### 视觉层次
根据 Windows 11 的设计规范，阴影和轮廓用于表达提升感：
| 元素 | 提升值 | 描边宽度 |
|------|--------|----------|
| 控件 (Control) | 2 | 1px |
| 卡片 (Card) | 8 | 1px |
| 提示框 (Tooltip) | 16 | 1px |
| 弹窗 (Flyout) | 32 | 1px |
| 对话框 (Dialog) | 128 | 1px |
| 窗口 (Window) | 128 | 1px |

---

## 一、抓手光标 (Grab/Grabbing)

### 1.1 设计规范

根据 Microsoft 文档，抓手光标用于在固定画布内平移内容（如地图）。Windows 定义了两种状态：

| 状态 | 描述 | 用途 |
|------|------|------|
| **Open Hand (Grab)** | 张开的手形 | 表示可抓取/可拖动 |
| **Closed Hand (Grabbing)** | 握紧的手形 | 表示正在抓取/拖动中 |

### 1.2 视觉规格

**形状特征：**
- 手掌部分呈弧形，类似真实手形
- 5 个手指清晰可辨
- 拇指与其他手指相对
- 轮廓线使用 1px 白色描边

**颜色规范：**
- **填充**：纯黑色 (#000000)
- **描边**：纯白色 (#FFFFFF)
- **热点位置**：手掌中心（约 16, 16 像素位置）

**尺寸：**
- 画布：32x32 像素
- 手形主体：约 20x18 像素
- 手指宽度：约 3-4 像素

### 1.3 SVG 代码实现

#### Grab（张开手）光标

```svg
<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32" viewBox="0 0 32 32">
  <!-- 热点定义：手掌中心 -->
  <defs>
    <filter id="shadow" x="-20%" y="-20%" width="140%" height="140%">
      <feDropShadow dx="0" dy="1" stdDeviation="0.5" flood-color="rgba(0,0,0,0.3)"/>
    </filter>
  </defs>
  
  <!-- 白色描边轮廓 -->
  <g fill="none" stroke="#FFFFFF" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
    <!-- 手掌主体 -->
    <path d="M10 18 C10 18 9 14 11 12 C13 10 15 11 16 13 L16 10 C16 8 18 7 20 8 C21 9 22 10 21 12 L22 11 C24 10 26 11 26 13 C27 15 26 17 24 18"/>
    <!-- 拇指 -->
    <path d="M10 18 C10 18 8 16 9 14 C10 12 12 13 13 15"/>
    <!-- 食指 -->
    <path d="M16 13 C16 11 17 9 19 9 C21 9 22 11 21 13"/>
    <!-- 中指 -->
    <path d="M19 14 C19 12 20 10 22 10 C24 10 25 12 24 14"/>
    <!-- 无名指 -->
    <path d="M22 15 C22 13 23 11 25 12 C27 12 27 14 26 16"/>
    <!-- 小指 -->
    <path d="M24 18 C25 16 26 15 28 16 C29 17 29 19 27 20"/>
  </g>
  
  <!-- 黑色填充主体 -->
  <g fill="#000000" stroke="none">
    <!-- 手掌 -->
    <path d="M10 18 C10 18 9 14 11 12 C13 10 15 11 16 13 L16 10 C16 8 18 7 20 8 C21 9 22 10 21 12 L22 11 C24 10 26 11 26 13 C27 15 26 17 24 18 L24 22 C24 25 21 27 18 27 L14 27 C11 27 10 25 10 22 Z"/>
    <!-- 拇指 -->
    <path d="M10 18 C10 18 8 16 9 14 C10 12 12 13 13 15 L14 18"/>
    <!-- 手指 -->
    <ellipse cx="18" cy="11" rx="2" ry="3"/>
    <ellipse cx="21" cy="12" rx="2" ry="3"/>
    <ellipse cx="24" cy="14" rx="2" ry="3"/>
    <ellipse cx="26" cy="17" rx="2" ry="3"/>
  </g>
  
  <!-- 热点标记（实际使用时删除） -->
  <circle cx="16" cy="16" r="1" fill="red" opacity="0.5"/>
</svg>
```

#### Grabbing（握紧手）光标

```svg
<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32" viewBox="0 0 32 32">
  <!-- 白色描边轮廓 -->
  <g fill="none" stroke="#FFFFFF" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
    <!-- 握紧的手形 -->
    <path d="M10 18 C10 18 9 14 11 12 C13 10 15 11 16 13"/>
    <!-- 握成拳头的轮廓 -->
    <path d="M10 18 L10 22 C10 25 12 27 15 27 L19 27 C22 27 24 25 24 22 L24 18"/>
    <!-- 手指关节轮廓 -->
    <path d="M13 15 C13 13 14 11 16 11 C18 11 19 13 19 15"/>
    <path d="M16 16 C16 14 17 12 19 12 C21 12 22 14 22 16"/>
    <path d="M19 17 C19 15 20 13 22 13 C24 13 25 15 25 17"/>
    <path d="M22 18 C22 16 23 14 25 15 C27 15 27 17 26 19"/>
  </g>
  
  <!-- 黑色填充主体 -->
  <g fill="#000000" stroke="none">
    <!-- 握紧的拳头 -->
    <path d="M10 18 L10 22 C10 25 12 27 15 27 L19 27 C22 27 24 25 24 22 L24 18 C26 17 27 15 26 13 C26 11 24 10 22 11 L21 12 C22 10 21 9 20 8 C18 7 16 8 16 10 L16 13 C15 11 13 10 11 12 C9 14 10 18 10 18 Z"/>
    <!-- 指关节细节 -->
    <circle cx="15" cy="14" r="2"/>
    <circle cx="18" cy="15" r="2"/>
    <circle cx="21" cy="16" r="2"/>
    <circle cx="24" cy="17" r="2"/>
  </g>
  
  <!-- 热点标记（实际使用时删除） -->
  <circle cx="16" cy="16" r="1" fill="red" opacity="0.5"/>
</svg>
```

#### 简化的 Fluent Design 风格抓手光标

```svg
<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32" viewBox="0 0 32 32">
  <!-- 白色描边层（确保在深色背景可见） -->
  <path fill="none" stroke="#FFFFFF" stroke-width="2" stroke-linejoin="round"
    d="M8 20 C7 17 8 14 10 13 C12 12 14 13 15 15 L15 11 C15 9 17 7 19 8 C21 9 21 11 20 13 L22 12 C24 11 26 12 26 14 C27 16 26 18 24 19 L24 23 C24 26 22 28 19 28 L13 28 C10 28 8 26 8 23 Z"/>
  
  <!-- 黑色填充主体 -->
  <path fill="#000000" stroke="none"
    d="M8 20 C7 17 8 14 10 13 C12 12 14 13 15 15 L15 11 C15 9 17 7 19 8 C21 9 21 11 20 13 L22 12 C24 11 26 12 26 14 C27 16 26 18 24 19 L24 23 C24 26 22 28 19 28 L13 28 C10 28 8 26 8 23 Z"/>
  
  <!-- 手指线条细节 -->
  <g stroke="#FFFFFF" stroke-width="0.5" stroke-linecap="round" opacity="0.5">
    <path d="M11 14 L11 16"/>
    <path d="M14 14 L14 16"/>
    <path d="M17 13 L17 15"/>
    <path d="M20 14 L20 16"/>
  </g>
</svg>
```

---

## 二、工字光标 (Text/I-beam)

### 2.1 设计规范

根据 Microsoft 文档，工字光标（I-beam 或 Text Select）用于表示文本可选择的位置。

**用途：**
- 表示可输入文本的区域
- 指示字符之间的插入点位置
- 用于所有文本选择操作

### 2.2 视觉规格

**形状特征：**
- 垂直线条为主体
- 顶部和底部有水平横线（类似 "I" 字形）
- 横线略宽于垂直线，形成工字形状
- 整体呈细长形状

**颜色规范：**
- **填充**：纯黑色 (#000000)
- **描边**：纯白色 (#FFFFFF)，1px 宽度
- **热点位置**：线条中点（约 16, 16 像素位置）

**尺寸：**
- 画布：32x32 像素
- 垂直线：宽度 2px，高度 24px
- 横线：宽度 8px，高度 2px
- 整体尺寸：约 8x24 像素

### 2.3 SVG 代码实现

#### 标准工字光标

```svg
<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32" viewBox="0 0 32 32">
  <!-- 白色描边背景（确保在深色背景可见） -->
  <g fill="none" stroke="#FFFFFF" stroke-width="2" stroke-linecap="round">
    <!-- 顶部横线 -->
    <line x1="12" y1="6" x2="20" y2="6"/>
    <!-- 底部横线 -->
    <line x1="12" y1="26" x2="20" y2="26"/>
    <!-- 垂直线 -->
    <line x1="16" y1="6" x2="16" y2="26"/>
  </g>
  
  <!-- 黑色填充主体 -->
  <g fill="#000000" stroke="none">
    <!-- 顶部横线 -->
    <rect x="12" y="5" width="8" height="3" rx="0.5"/>
    <!-- 底部横线 -->
    <rect x="12" y="24" width="8" height="3" rx="0.5"/>
    <!-- 垂直线 -->
    <rect x="15" y="6" width="2" height="20" rx="0.5"/>
  </g>
  
  <!-- 热点标记（实际使用时删除） -->
  <circle cx="16" cy="16" r="1" fill="red" opacity="0.5"/>
</svg>
```

#### Windows 11 风格的工字光标（圆角现代风格）

```svg
<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32" viewBox="0 0 32 32">
  <defs>
    <!-- 轻微阴影增加深度感 -->
    <filter id="cursorShadow" x="-50%" y="-50%" width="200%" height="200%">
      <feDropShadow dx="0" dy="1" stdDeviation="0.5" flood-color="rgba(0,0,0,0.4)"/>
    </filter>
  </defs>
  
  <!-- 白色描边层 -->
  <g fill="none" stroke="#FFFFFF" stroke-width="1.5" stroke-linecap="round" filter="url(#cursorShadow)">
    <!-- 顶部横线 -->
    <line x1="12" y1="6" x2="20" y2="6"/>
    <!-- 底部横线 -->
    <line x1="12" y1="26" x2="20" y2="26"/>
    <!-- 垂直线 -->
    <line x1="16" y1="6" x2="16" y2="26"/>
  </g>
  
  <!-- 黑色填充主体 -->
  <g fill="#1A1A1A" stroke="none">
    <!-- 顶部横线（圆角） -->
    <rect x="12" y="5.5" width="8" height="2" rx="1"/>
    <!-- 底部横线（圆角） -->
    <rect x="12" y="24.5" width="8" height="2" rx="1"/>
    <!-- 垂直线（圆角） -->
    <rect x="15.25" y="6" width="1.5" height="20" rx="0.75"/>
  </g>
</svg>
```

#### 简化版工字光标

```svg
<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32" viewBox="0 0 32 32">
  <!-- 描边填充一体化设计 -->
  <g fill="#000000" stroke="#FFFFFF" stroke-width="1.5" stroke-linejoin="round">
    <path d="M13 6 L19 6 L19 7.5 L16.75 7.5 L16.75 24.5 L19 24.5 L19 26 L13 26 L13 24.5 L15.25 24.5 L15.25 7.5 L13 7.5 Z"/>
  </g>
</svg>
```

---

## 三、动态加载方案分析

### 3.1 实现方式

在 Web 应用中使用自定义光标主要有以下几种方案：

#### 方案一：CSS URL 引用（推荐简单场景）

```css
/* 基础用法 */
.grab-cursor {
  cursor: url('/cursors/grab.svg'), grab;
}

.grabbing-cursor {
  cursor: url('/cursors/grabbing.svg'), grabbing;
}

.text-cursor {
  cursor: url('/cursors/text.svg'), text;
}
```

**优点：**
- 简单易用，一行代码实现
- 浏览器自动处理加载和缓存
- 支持降级（提供系统光标作为后备）

**缺点：**
- 需要 HTTP 请求加载 SVG 文件
- 首次加载可能有延迟
- 不易动态修改光标样式

#### 方案二：Data URI 内联（零延迟）

```css
.grab-cursor {
  cursor: url('data:image/svg+xml;utf8,<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32" viewBox="0 0 32 32"><path fill="%23000" stroke="%23FFF" stroke-width="2" d="M10 18c0 0-1-4 1-6s4-1 5 1l0-4c0-2 2-3 4-2s2 2 1 4l1-1c2-1 4 0 4 2s-1 4-2 5l0 4c0 3-3 5-6 5l-4 0c-3 0-4-2-4-5z"/></svg>'), grab;
}
```

**优点：**
- 零网络延迟，立即显示
- 不依赖外部文件
- 适合单页应用

**缺点：**
- CSS 文件体积增大
- 不易维护（SVG 代码直接写在 CSS 中）
- 编码复杂（需要 URL encode 特殊字符）

#### 方案三：JavaScript 动态加载（高级场景）

```javascript
// cursor-loader.js
class CursorManager {
  constructor() {
    this.cursorCache = new Map();
    this.basePath = '/cursors/';
  }

  // 预加载光标
  async preload(cursors) {
    const promises = cursors.map(name => this.loadCursor(name));
    return Promise.all(promises);
  }

  // 加载单个光标
  async loadCursor(name) {
    if (this.cursorCache.has(name)) {
      return this.cursorCache.get(name);
    }

    try {
      const response = await fetch(`${this.basePath}${name}.svg`);
      const svgText = await response.text();
      const dataUri = this.svgToDataUri(svgText);
      this.cursorCache.set(name, dataUri);
      return dataUri;
    } catch (error) {
      console.error(`Failed to load cursor: ${name}`, error);
      return null;
    }
  }

  // SVG 转 Data URI
  svgToDataUri(svgText) {
    const encoded = encodeURIComponent(svgText)
      .replace(/%20/g, ' ')
      .replace(/%3D/g, '=')
      .replace(/%3A/g, ':')
      .replace(/%2F/g, '/');
    return `data:image/svg+xml;utf8,${encoded}`;
  }

  // 应用到元素
  async apply(element, cursorName, fallback = 'auto') {
    const dataUri = await this.loadCursor(cursorName);
    if (dataUri) {
      element.style.cursor = `${dataUri}, ${fallback}`;
    } else {
      element.style.cursor = fallback;
    }
  }
}

// 使用示例
const cursorManager = new CursorManager();

// 预加载
await cursorManager.preload(['grab', 'grabbing', 'text']);

// 应用到元素
const draggableElement = document.getElementById('draggable');
draggableElement.addEventListener('mouseenter', () => {
  cursorManager.apply(draggableElement, 'grab', 'grab');
});
draggableElement.addEventListener('mousedown', () => {
  cursorManager.apply(draggableElement, 'grabbing', 'grabbing');
});
```

**优点：**
- 按需加载，节省带宽
- 可缓存，重复使用方便
- 支持动态主题切换
- 错误处理完善

**缺点：**
- 实现复杂度较高
- 异步加载需要处理加载状态
- 需要额外 JavaScript 代码

#### 方案四：React/Vue 组件化方案

```typescript
// React Hook 示例
import { useState, useEffect, useCallback } from 'react';

interface CursorOptions {
  fallback?: string;
  hotspot?: { x: number; y: number };
}

export function useCustomCursor(cursorName: string, options: CursorOptions = {}) {
  const { fallback = 'auto', hotspot = { x: 16, y: 16 } } = options;
  const [cursorUrl, setCursorUrl] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    const loadCursor = async () => {
      try {
        // 从模块或 URL 加载
        const cursorModule = await import(`./cursors/${cursorName}.svg?raw`);
        const svgText = cursorModule.default;
        
        // 修改热点位置
        const modifiedSvg = svgText.replace(
          'viewBox="0 0 32 32"',
          `viewBox="0 0 32 32"`
        );
        
        const encoded = btoa(modifiedSvg);
        const dataUri = `data:image/svg+xml;base64,${encoded}`;
        setCursorUrl(dataUri);
      } catch (error) {
        console.error('Cursor load failed:', error);
      } finally {
        setIsLoading(false);
      }
    };

    loadCursor();
  }, [cursorName]);

  const cursorStyle = cursorUrl 
    ? { cursor: `${cursorUrl} ${hotspot.x} ${hotspot.y}, ${fallback}` }
    : { cursor: fallback };

  return { cursorStyle, isLoading, cursorUrl };
}

// 使用
function DraggableComponent() {
  const { cursorStyle: grabStyle } = useCustomCursor('grab', { fallback: 'grab' });
  const { cursorStyle: grabbingStyle } = useCustomCursor('grabbing', { fallback: 'grabbing' });
  
  const [isDragging, setIsDragging] = useState(false);

  return (
    <div
      style={isDragging ? grabbingStyle : grabStyle}
      onMouseDown={() => setIsDragging(true)}
      onMouseUp={() => setIsDragging(false)}
    >
      Drag me
    </div>
  );
}
```

### 3.2 方案对比

| 方案 | 加载延迟 | 实现复杂度 | 维护性 | 适用场景 |
|------|----------|------------|--------|----------|
| CSS URL | 中等 | 低 | 高 | 静态项目 |
| Data URI | 无 | 低 | 低 | 小型单页应用 |
| JS 动态加载 | 可控 | 中 | 高 | 中大型应用 |
| 组件化方案 | 可控 | 高 | 高 | React/Vue 项目 |

### 3.3 最佳实践建议

1. **预加载关键光标**：在应用初始化时预加载主要光标，避免交互延迟

2. **提供系统后备**：始终提供系统光标作为后备
   ```css
   cursor: url('custom.svg'), grab;  /* 正确 */
   cursor: url('custom.svg');        /* 错误：无后备 */
   ```

3. **尺寸控制**：保持 32x32 标准尺寸，确保清晰度

4. **热点设置**：正确设置热点位置
   ```css
   cursor: url('custom.svg') 16 16, auto; /* 热点在中心 */
   ```

5. **暗色模式支持**：考虑提供反色版本
   ```javascript
   const isDarkMode = window.matchMedia('(prefers-color-scheme: dark)').matches;
   const cursorName = isDarkMode ? 'grab-light' : 'grab';
   ```

---

## 参考资料

1. [Microsoft Learn - Mouse Interactions](https://learn.microsoft.com/en-us/windows/apps/design/input/mouse-interactions)
2. [Microsoft Learn - About Cursors](https://learn.microsoft.com/en-us/windows/win32/menurc/about-cursors)
3. [Microsoft Learn - Windows 7 Mouse and Pointers](https://learn.microsoft.com/en-us/windows/win32/uxguide/inter-mouse)
4. [Microsoft Learn - Icons in Windows apps](https://learn.microsoft.com/en-us/windows/apps/design/style/icons)
5. [Microsoft Learn - Color in Windows](https://learn.microsoft.com/en-us/windows/apps/design/style/color)
6. [Microsoft Learn - Layering and elevation](https://learn.microsoft.com/en-us/windows/apps/design/signature-experiences/layering)
7. [Microsoft Fluent UI System Icons](https://github.com/microsoft/fluentui-system-icons)
8. [MDN - cursor CSS property](https://developer.mozilla.org/en-US/docs/Web/CSS/cursor)

---

**报告生成日期**: 2026年3月1日  
**适用系统**: Windows 10/11  
**设计规范版本**: Fluent Design System (2024)
