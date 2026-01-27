#!/bin/bash

# sync_common.sh
# Shared functions for sync_to_main.sh and sync_pr.sh
# This file should be sourced, not executed directly

# Colors for output
export GREEN='\033[0;32m'
export BLUE='\033[0;34m'
export YELLOW='\033[1;33m'
export RED='\033[0;31m'
export NC='\033[0m' # No Color

# Configuration
export CODECRAFTERS_BRANCH="codecrafters"
export MAIN_BRANCH="main"
export CC_FILES=(
    ".codecrafters"
    "codecrafters.yml"
    "codecrafters.yaml"
    "your_program.sh"
    "sync_to_main.sh"
    "sync_pr.sh"
    "sync_common.sh"
    ".gitattributes"
)

check_clean_state() {
    if [[ -n $(git status -s) ]]; then
        echo -e "${YELLOW}⚠️  You have uncommitted changes. Please commit or stash them first.${NC}"
        exit 1
    fi
}

update_codecrafters() {
    echo -e "${BLUE}📥 Fetching latest changes...${NC}"
    git fetch origin

    echo -e "${BLUE}🔄 Switching to ${CODECRAFTERS_BRANCH} branch...${NC}"
    git checkout "$CODECRAFTERS_BRANCH"
    git pull origin "$CODECRAFTERS_BRANCH"
}

push_to_remotes() {
    echo -e "${BLUE}📤 Pushing to origin...${NC}"
    git push origin "$CODECRAFTERS_BRANCH"

    if git remote | grep -q "^cc$"; then
        echo -e "${BLUE}📤 Pushing to CodeCrafters for validation...${NC}"
        git push cc codecrafters:master
        echo -e "${GREEN}✓${NC} CodeCrafters updated"
    else
        echo -e "${YELLOW}ℹ️  CodeCrafters remote 'cc' not found, skipping${NC}"
    fi
}

remove_cc_files() {
    echo -e "${BLUE}🧹 Removing CodeCrafters-specific files...${NC}"
    local files_removed=()
    for file in "${CC_FILES[@]}"; do
        if [[ -e "$file" ]]; then
            git rm -r "$file" 2>/dev/null || true
            files_removed+=("$file")
            echo -e "  ${GREEN}✓${NC} Removed: $file"
        fi
    done
}

update_cargo_toml() {
    echo -e "${BLUE}📝 Updating Cargo.toml...${NC}"
    if [[ -f "Cargo.toml" ]]; then
        # Extract the existing [dependencies] and other sections
        local dependencies_section=""
        if grep -q "^\[dependencies\]" Cargo.toml; then
            dependencies_section=$(sed -n '/^\[dependencies\]/,$p' Cargo.toml)
        fi

        # Get the edition field if it exists
        local edition=$(grep "^edition = " Cargo.toml | head -1 || echo 'edition = "2021"')

        # Create new Cargo.toml with standard field order
        cat > Cargo.toml << 'EOF'
[package]
name = "ferrish"
version = "0.1.0"
EOF
        echo "${edition}" >> Cargo.toml
        cat >> Cargo.toml << 'EOF'
authors = ["Carson Price <cdprice02@users.noreply.github.com>"]
description = "Ferrish is a modern, Rust-powered shell focused on safety, performance, and a clean interactive experience."
repository = "https://github.com/cdprice02/ferrish"
license = "MIT"

EOF

        # Append dependencies section if it existed
        if [[ -n "$dependencies_section" ]]; then
            echo "$dependencies_section" >> Cargo.toml
        else
            echo "[dependencies]" >> Cargo.toml
        fi

        git add Cargo.toml
        echo -e "  ${GREEN}✓${NC} Updated Cargo.toml metadata"
    fi
}

cargo_check() {
    echo -e "${BLUE}🔨 Running cargo check...${NC}"

    # Check if cargo is available
    if ! command -v cargo &> /dev/null; then
        echo -e "${RED}❌ Error: cargo command not found${NC}"
        echo -e "${YELLOW}Please install Rust toolchain: https://rustup.rs/${NC}"
        exit 1
    fi

    # Use cargo check to verify and update Cargo.lock
    # This is faster than cargo build and only verifies the project
    if cargo check --quiet 2>/dev/null; then
        echo -e "  ${GREEN}✓${NC} Cargo check passed"
    else
        echo -e "${YELLOW}⚠️  Warning: cargo check had issues, trying cargo build...${NC}"
        if cargo build --quiet 2>&1 | head -5; then
            echo -e "  ${GREEN}✓${NC} Cargo build completed"
        else
            echo -e "${RED}❌ Error: Cargo check/build failed${NC}"
            echo -e "${YELLOW}You may need to fix compilation errors first${NC}"
            exit 1
        fi
    fi

    # Stage the updated Cargo.lock
    if [[ -f "Cargo.lock" ]]; then
        git add Cargo.lock
        echo -e "  ${GREEN}✓${NC} Cargo.lock updated and staged"
    fi
}

sanitize_branch_name() {
    local name="$1"
    echo "$name" | tr '[:upper:]' '[:lower:]' | sed 's/[^a-z0-9-]/-/g' | sed 's/--*/-/g' | sed 's/^-//' | sed 's/-$//'
}

generate_pr_title() {
    local branch_name="$1"
    echo "$branch_name" | sed 's/-/ /g' | awk '{for(i=1;i<=NF;i++)sub(/./,toupper(substr($i,1,1)),$i)}1'
}

generate_pr_url() {
    local branch_name="$1"
    local encoded_branch=$(echo "$branch_name" | sed 's/\//%2F/g')
    local pr_title=$(generate_pr_title "$branch_name")
    local encoded_title=$(echo "$pr_title" | sed 's/ /%20/g')

    echo "https://github.com/cdprice02/ferrish/compare/${MAIN_BRANCH}...${encoded_branch}?expand=1&title=${encoded_title}"
}

open_in_browser() {
    local url="$1"

    if command -v xdg-open &> /dev/null; then
        read -p "Open PR URL in browser? (y/n) " -n 1 -r
        echo
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            xdg-open "$url"
        fi
    elif command -v open &> /dev/null; then
        read -p "Open PR URL in browser? (y/n) " -n 1 -r
        echo
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            open "$url"
        fi
    fi
}

delete_branch_if_exists() {
    local branch_name="$1"
    local deleted=false

    # Check if branch exists locally
    if git show-ref --verify --quiet "refs/heads/${branch_name}"; then
        echo -e "${BLUE}��️  Deleting local branch: ${branch_name}${NC}"
        git branch -D "$branch_name"
        echo -e "${GREEN}✓${NC} Local branch deleted"
        deleted=true
    fi

    # Check if branch exists remotely
    if git ls-remote --heads origin "$branch_name" | grep -q "$branch_name"; then
        echo -e "${BLUE}🗑️  Deleting remote branch: ${branch_name}${NC}"
        git push origin --delete "$branch_name"
        echo -e "${GREEN}✓${NC} Remote branch deleted"
        deleted=true
    fi

    if [[ "$deleted" = false ]]; then
        echo -e "${YELLOW}ℹ️  Branch '${branch_name}' not found${NC}"
    fi
}

create_sync_branch() {
    local branch_name="$1"

    echo -e "${BLUE}🌿 Creating sync branch: ${branch_name}${NC}"
    git checkout -b "$branch_name"

    remove_cc_files
    update_cargo_toml
    cargo_check

    # Check if there are any changes to commit
    if [[ -n $(git status -s) ]]; then
        git commit -m "Clean up for portfolio sync

- Remove CodeCrafters-specific files
- Update Cargo.toml with proper project metadata
- Update Cargo.lock to match"
        echo -e "${GREEN}✓${NC} Changes committed"
    else
        echo -e "${YELLOW}ℹ️  No changes to commit${NC}"
    fi

    # Push sync branch
    echo -e "${BLUE}📤 Pushing sync branch to origin...${NC}"
    git push origin "$branch_name"
}

display_pr_info() {
    local branch_name="$1"
    local pr_url="$2"
    local pr_title=$(generate_pr_title "$branch_name")

    echo ""
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${BLUE}📝 Pull Request Details:${NC}"
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo ""
    echo -e "  ${GREEN}Base:${NC}           ${MAIN_BRANCH}"
    echo -e "  ${GREEN}Compare:${NC}        ${branch_name}"
    echo -e "  ${GREEN}Title:${NC}          ${pr_title}"
    echo ""
    echo -e "  ${YELLOW}🔗 ${pr_url}${NC}"
    echo ""
}
