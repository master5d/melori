import React from "react";

const EchoTextLogo = ({
  width,
  height,
  className,
}: {
  width?: number;
  height?: number;
  className?: string;
}) => {
  return (
    <div
      className={`flex flex-col items-center justify-center ${className}`}
      style={{ width, height }}
    >
      <h1 className="text-4xl font-bold tracking-tighter text-accent">ECHO</h1>
      <div className="mt-1 h-1 w-12 bg-accent rounded-full"></div>
    </div>
  );
};

export default EchoTextLogo;
