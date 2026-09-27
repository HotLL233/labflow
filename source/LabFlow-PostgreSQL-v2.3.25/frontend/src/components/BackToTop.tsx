import React, { useState, useEffect } from 'react';
import { Fab, Zoom } from '@mui/material';
import KeyboardArrowUpIcon from '@mui/icons-material/KeyboardArrowUp';

const BackToTop: React.FC = () => {
  const [visible, setVisible] = useState(false);

  useEffect(() => {
    const handleScroll = () => {
      const documentHeight = document.documentElement.scrollHeight;
      const nearBottom = window.innerHeight + window.scrollY >= documentHeight - 180;
      // 到达列表底部时隐藏，避免遮挡分页；滚动离开底部后再恢复。
      setVisible(window.scrollY > 200 && !nearBottom);
    };
    window.addEventListener('scroll', handleScroll, { passive: true });
    window.addEventListener('resize', handleScroll, { passive: true });
    handleScroll();
    return () => {
      window.removeEventListener('scroll', handleScroll);
      window.removeEventListener('resize', handleScroll);
    };
  }, []);

  const scrollToTop = () => {
    window.scrollTo({ top: 0, behavior: 'smooth' });
  };

  return (
    <Zoom in={visible}>
      <Fab
        color="primary"
        size="small"
        onClick={scrollToTop}
        sx={{
          position: 'fixed',
          bottom: { xs: 82, sm: 24 },
          right: { xs: 12, sm: 24 },
          zIndex: 1000,
          bgcolor: '#2e7d32',
          '&:hover': { bgcolor: '#1b5e20' },
          width: 40,
          height: 40,
        }}
      >
        <KeyboardArrowUpIcon />
      </Fab>
    </Zoom>
  );
};

export default BackToTop;
