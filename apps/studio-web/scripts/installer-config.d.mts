// Types of installer-config.mjs for tests/unit/installers.spec.ts (the scripts are plain JavaScript).
export declare const PRODUCT_NAME: string;
export declare const APP_ID: string;
export declare const EXECUTABLE: string;
export declare const INSTALLER_METADATA: { description: string; author: { name: string; url: string }; homepage: string };
export declare function runtimePackage(pkg: Record<string, unknown>): Record<string, unknown>;
export declare function builderConfig(o: {
  pkg: { devDependencies: Record<string, string> } & Record<string, unknown>;
  stage: string;
  output: string;
  sign: boolean;
}): Record<string, any>;
