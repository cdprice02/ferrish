#!/bin/bash

# sync_to_main.sh
# Automates creating a PR to sync codecrafters branch to main (excluding CC files)
# Usage: ./sync_to_main.sh [branch-name]

set -e  # Exit on error

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Configuration
CODECRAFTERS_BRANCH="codecrafters"
MAIN_BRANCH="main"
CC_FILES=(
    ".gitattributes"
    ".codecrafters"
    "codecrafters.yml"
    "codecrafters.yaml"
    "your_program.sh"
    "sync_to_main.sh"
)

# Get current branch
CURRENT_BRANCH=$(git branch --show-current)

echo -e "${BLUE}🚀 Starting sync from ${CODECRAFTERS_BRANCH} to ${MAIN_BRANCH}${NC}"
echo ""

# Get custom name from argument or prompt
if [[ -n "$1" ]]; then
    CUSTOM_NAME="$1"
    echo -e "${BLUE}Using branch name:${NC} ${CUSTOM_NAME}"
else
    echo -e "${YELLOW}Enter a name for this sync (e.g., 'add-grep', 'implement-pipes'):${NC}"
    read -p "> " CUSTOM_NAME
fi

# Validate input
if [[ -z "$CUSTOM_NAME" ]]; then
    echo -e "${RED}❌ Branch name cannot be empty${NC}"
    exit 1
fi

# Sanitize branch name (replace spaces with hyphens, remove special chars)
CUSTOM_NAME=$(echo "$CUSTOM_NAME" | tr '[:upper:]' '[:lower:]' | sed 's/[^a-z0-9-]/-/g' | sed 's/--*/-/g' | sed 's/^-//' | sed 's/-$//')

echo ""
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

# Ensure we're on a clean state
if [[ -n $(git status -s) ]]; then
    echo -e "${YELLOW}⚠️  You have uncommitted changes. Please commit or stash them first.${NC}"
    exit 1
fi

# Ensure codecrafters branch exists and is up to date
echo -e "${BLUE}📥 Fetching latest changes...${NC}"
git fetch origin

# Switch to codecrafters branch
echo -e "${BLUE}🔄 Switching to ${CODECRAFTERS_BRANCH} branch...${NC}"
git checkout "$CODECRAFTERS_BRANCH"
git pull origin "$CODECRAFTERS_BRANCH"

# Use the custom name directly as the branch name
SYNC_BRANCH="${CUSTOM_NAME}"

echo -e "${BLUE}🌿 Creating sync branch: ${SYNC_BRANCH}${NC}"

# Check if branch already exists
if git show-ref --verify --quiet "refs/heads/${SYNC_BRANCH}"; then
    echo -e "${RED}❌ Branch '${SYNC_BRANCH}' already exists locally${NC}"
    echo -e "${YELLOW}Please delete it first or choose a different name:${NC}"
    echo -e "   git branch -D ${SYNC_BRANCH}"
    exit 1
fi

git checkout -b "$SYNC_BRANCH"

# Remove CodeCrafters-specific files
echo -e "${BLUE}🧹 Removing CodeCrafters-specific files...${NC}"
FILES_REMOVED=()
for file in "${CC_FILES[@]}"; do
    if [[ -e "$file" ]]; then
        git rm -r "$file" 2>/dev/null || true
        FILES_REMOVED+=("$file")
        echo -e "  ${GREEN}✓${NC} Removed: $file"
    fi
done

# Update Cargo.toml with proper project info
echo -e "${BLUE}📝 Updating Cargo.toml...${NC}"
if [[ -f "Cargo.toml" ]]; then
    # Extract the existing [dependencies] and other sections
    DEPENDENCIES_SECTION=""
    if grep -q "^\[dependencies\]" Cargo.toml; then
        DEPENDENCIES_SECTION=$(sed -n '/^\[dependencies\]/,$p' Cargo.toml)
    fi

    # Get the edition field if it exists
    EDITION=$(grep "^edition = " Cargo.toml | head -1 || echo 'edition = "2021"')

    # Create new Cargo.toml with standard field order
    cat > Cargo.toml << EOF
[package]
name = "ferrish"
version = "0.1.0"
${EDITION}
authors = ["Carson Price <cdprice02@users.noreply.github.com>"]
description = "Ferrish is a modern, Rust-powered shell focused on safety, performance, and a clean interactive experience."
repository = "https://github.com/cdprice02/ferrish"
license = "MIT"

EOF

    # Append dependencies section if it existed
    if [[ -n "$DEPENDENCIES_SECTION" ]]; then
        echo "$DEPENDENCIES_SECTION" >> Cargo.toml
    else
        echo "[dependencies]" >> Cargo.toml
    fi

    git add Cargo.toml
    echo -e "  ${GREEN}✓${NC} Updated Cargo.toml metadata"
fi

# Check if there are any changes to commit
if [[ -n $(git status -s) ]]; then
    git commit -m "Clean up for portfolio sync

- Remove CodeCrafters-specific files
- Update Cargo.toml with proper project metadata"
    echo -e "${GREEN}✓${NC} Changes committed"
else
    echo -e "${YELLOW}ℹ️  No changes to commit${NC}"
fi

# Push sync branch
echo -e "${BLUE}📤 Pushing sync branch to origin...${NC}"
git push origin "$SYNC_BRANCH"

# Generate PR URL with URL-encoded branch name and title
ENCODED_SYNC_BRANCH=$(echo "$SYNC_BRANCH" | sed 's/\//%2F/g')

# Generate suggested PR title (capitalize first letter of each word)
PR_TITLE=$(echo "$CUSTOM_NAME" | sed 's/-/ /g' | awk '{for(i=1;i<=NF;i++)sub(/./,toupper(substr($i,1,1)),$i)}1')

# URL-encode the PR title for the query parameter
ENCODED_PR_TITLE=$(echo "$PR_TITLE" | sed 's/ /%20/g')

PR_URL="https://github.com/cdprice02/ferrish/compare/${MAIN_BRANCH}...${ENCODED_SYNC_BRANCH}?expand=1&title=${ENCODED_PR_TITLE}"

echo ""
echo -e "${GREEN}✅ Sync branch created and pushed successfully!${NC}"
echo ""
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${BLUE}📝 Create your Pull Request:${NC}"
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo ""
echo -e "  ${GREEN}Base:${NC}           ${MAIN_BRANCH}"
echo -e "  ${GREEN}Compare:${NC}        ${SYNC_BRANCH}"
echo -e "  ${GREEN}Title:${NC}          ${PR_TITLE}"
echo ""
echo -e "  ${YELLOW}🔗 ${PR_URL}${NC}"
echo ""

# Try to open in browser (optional)
if command -v xdg-open &> /dev/null; then
    read -p "Open PR URL in browser? (y/n) " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        xdg-open "$PR_URL"
    fi
elif command -v open &> /dev/null; then
    read -p "Open PR URL in browser? (y/n) " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        open "$PR_URL"
    fi
fi

# Return to original branch
echo -e "${BLUE}🔄 Returning to ${CURRENT_BRANCH} branch...${NC}"
git checkout "$CURRENT_BRANCH"

echo ""
echo -e "${GREEN}✨ Sync complete!${NC}"
echo ""
echo -e "${BLUE}Next steps:${NC}"
echo -e "  1. ${YELLOW}Click the URL above${NC} (or copy/paste it into your browser)"
echo -e "  2. Title will be pre-filled: ${GREEN}${PR_TITLE}${NC}"
echo -e "  3. Review the changes and add a description"
echo -e "  4. Click ${GREEN}'Create pull request'${NC}"
echo -e "  5. Merge when ready"
echo ""
