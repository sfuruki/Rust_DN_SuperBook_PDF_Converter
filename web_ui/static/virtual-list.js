/**
 * Virtual List Implementation for Large Page Count Support
 * Purpose: Support 1000+ page PDFs with efficient DOM rendering
 * Strategy: Only render visible rows, recycle DOM nodes for performance
 */

class VirtualList {
  constructor(options = {}) {
    this.container = options.container;
    this.itemHeight = options.itemHeight || 32;
    this.visibleCount = options.visibleCount || 20;
    this.items = [];
    this.startIndex = 0;
    this.scrolling = false;

    if (!this.container) {
      throw new Error('VirtualList requires a container element');
    }

    this.scrollContainer = document.createElement('div');
    this.scrollContainer.className = 'virtual-scroll-container';
    this.scrollContainer.style.cssText = `
      height: ${this.itemHeight * this.visibleCount}px;
      overflow-y: auto;
      border: 1px solid #e0e0e0;
      border-radius: 4px;
      background: white;
    `;

    this.viewportDiv = document.createElement('div');
    this.viewportDiv.className = 'virtual-viewport';

    this.spacerTop = document.createElement('div');
    this.spacerTop.className = 'virtual-spacer-top';

    this.contentDiv = document.createElement('div');
    this.contentDiv.className = 'virtual-content';

    this.spacerBottom = document.createElement('div');
    this.spacerBottom.className = 'virtual-spacer-bottom';

    this.viewportDiv.appendChild(this.spacerTop);
    this.viewportDiv.appendChild(this.contentDiv);
    this.viewportDiv.appendChild(this.spacerBottom);
    this.scrollContainer.appendChild(this.viewportDiv);
    this.container.appendChild(this.scrollContainer);

    this.scrollContainer.addEventListener('scroll', () => this.handleScroll());
  }

  setItems(items) {
    this.items = items || [];
    this.startIndex = 0;
    this.render();
  }

  handleScroll() {
    if (this.scrolling) return;
    this.scrolling = true;

    requestAnimationFrame(() => {
      const scrollTop = this.scrollContainer.scrollTop;
      const newStartIndex = Math.floor(scrollTop / this.itemHeight);
      
      if (newStartIndex !== this.startIndex) {
        this.startIndex = Math.max(0, newStartIndex);
        this.render();
      }
      
      this.scrolling = false;
    });
  }

  render() {
    const endIndex = Math.min(this.startIndex + this.visibleCount, this.items.length);
    const visibleItems = this.items.slice(this.startIndex, endIndex);

    // Update spacers
    this.spacerTop.style.height = (this.startIndex * this.itemHeight) + 'px';
    this.spacerBottom.style.height = ((this.items.length - endIndex) * this.itemHeight) + 'px';

    // Clear content
    this.contentDiv.innerHTML = '';

    // Render visible items
    visibleItems.forEach((item, idx) => {
      const row = document.createElement('div');
      row.className = 'virtual-row';
      row.style.cssText = `
        height: ${this.itemHeight}px;
        line-height: ${this.itemHeight}px;
        padding: 0 12px;
        border-bottom: 1px solid #f0f0f0;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
      `;
      
      if (item.status === 'completed') {
        row.style.color = '#4caf50';
      } else if (item.status === 'error') {
        row.style.color = '#f44336';
      } else if (item.status === 'running') {
        row.style.color = '#4a90d9';
      }
      
      row.textContent = item.label || `Item ${this.startIndex + idx + 1}`;
      this.contentDiv.appendChild(row);
    });
  }

  addItem(item) {
    this.items.push(item);
    if (this.items.length === 1) {
      this.render();
    }
  }

  updateItem(index, item) {
    if (index >= 0 && index < this.items.length) {
      this.items[index] = { ...this.items[index], ...item };
      if (index >= this.startIndex && index < this.startIndex + this.visibleCount) {
        this.render();
      }
    }
  }

  clear() {
    this.items = [];
    this.startIndex = 0;
    this.render();
  }
}

// Export for browser
if (typeof module !== 'undefined' && module.exports) {
  module.exports = VirtualList;
}
