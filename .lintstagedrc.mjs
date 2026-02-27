export default {
	"*.{md,mdx}": () => "just fmt-md",
	"*.rs": () => "cargo fmt",
};
