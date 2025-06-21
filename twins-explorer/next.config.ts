import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  /* config options here */
  env: {
    TIMESTAMP: Date.now().toString(), // Force re-evaluation
  },
};

export default nextConfig;
